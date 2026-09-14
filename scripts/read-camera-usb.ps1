param([string]$OutputDirectory = 'output/camera-characterization/usb')
$ErrorActionPreference = 'Stop'
$taskInstance = 'USB\VID_0C45&PID_6369\SN0001'
$taskParent = (Get-PnpDeviceProperty -InstanceId $taskInstance -KeyName DEVPKEY_Device_Parent).Data
$taskLocation = (Get-PnpDeviceProperty -InstanceId $taskInstance -KeyName DEVPKEY_Device_LocationInfo).Data
if ($taskLocation -notmatch 'Port_#(\d+)') { throw 'Cannot determine camera hub port' }
$taskPort = [uint32]$Matches[1]
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class CameraUsbDescriptors {
 [DllImport("cfgmgr32.dll", CharSet=CharSet.Unicode)] static extern int CM_Get_Device_Interface_List_SizeW(out uint len, ref Guid guid, string id, uint flags);
 [DllImport("cfgmgr32.dll", CharSet=CharSet.Unicode)] static extern int CM_Get_Device_Interface_ListW(ref Guid guid, string id, [Out] char[] list, uint len, uint flags);
 [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern SafeFileHandle CreateFileW(string name, uint access, uint share, IntPtr sec, uint creation, uint flags, IntPtr template);
 [DllImport("kernel32.dll", SetLastError=true)] static extern bool DeviceIoControl(SafeFileHandle handle,uint code,byte[] input,uint inputSize,[Out] byte[] output,uint outputSize,out uint returned,IntPtr overlapped);
 public static string HubPath(string id) {
  Guid guid=new Guid("f18a0e88-c30c-11d0-8815-00a0c906bed8"); uint len;
  int status=CM_Get_Device_Interface_List_SizeW(out len,ref guid,id,0);
  if(status!=0) throw new Exception("Hub interface size: "+status);
  var chars=new char[len]; status=CM_Get_Device_Interface_ListW(ref guid,id,chars,len,0);
  if(status!=0) throw new Exception("Hub interface list: "+status);
  var paths=new string(chars).Split(new char[]{'\0'},StringSplitOptions.RemoveEmptyEntries);
  if(paths.Length!=1) throw new Exception("Ambiguous or absent parent hub interface");
  return paths[0];
 }
 static byte[] Query(string path,uint code,byte[] request) {
  // Zero desired access: only FILE_ANY_ACCESS read/query IOCTLs below.
  using(var handle=CreateFileW(path,0,3,IntPtr.Zero,3,0,IntPtr.Zero)) {
   if(handle.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
   var output=new byte[request.Length]; uint returned;
   if(!DeviceIoControl(handle,code,request,(uint)request.Length,output,(uint)output.Length,out returned,IntPtr.Zero)) throw new Win32Exception(Marshal.GetLastWin32Error());
   Array.Resize(ref output,(int)returned); return output;
  }
 }
 public static byte[] Connection(string path,uint port) {
  var req=new byte[4096]; BitConverter.GetBytes(port).CopyTo(req,0);
  return Query(path,(0x22u<<16)|(274u<<2),req);
 }
 public static byte[] Descriptor(string path,uint port,byte type,int count) {
  var req=new byte[12+count]; BitConverter.GetBytes(port).CopyTo(req,0);
  req[4]=0x80; req[5]=6; req[7]=type; BitConverter.GetBytes((ushort)count).CopyTo(req,10);
  var response=Query(path,(0x22u<<16)|(260u<<2),req);
  if(response.Length<12) throw new Exception("Truncated USB_DESCRIPTOR_REQUEST");
  var data=new byte[response.Length-12]; Array.Copy(response,12,data,0,data.Length); return data;
 }
}
'@
$taskHub = [CameraUsbDescriptors]::HubPath($taskParent)
$taskConnection = [CameraUsbDescriptors]::Connection($taskHub,$taskPort)
if ($taskConnection.Length -lt 35) { throw 'Truncated USB connection information' }
if ([BitConverter]::ToUInt16($taskConnection,12) -ne 0x0c45 -or [BitConverter]::ToUInt16($taskConnection,14) -ne 0x6369) { throw 'Hub port does not match CM678; refusing further queries' }
[IO.File]::WriteAllBytes((Join-Path $PWD "$OutputDirectory/connection.bin"),$taskConnection)
$taskConfigHeader = [CameraUsbDescriptors]::Descriptor($taskHub,$taskPort,2,9)
if ($taskConfigHeader.Length -lt 9) { throw 'Truncated configuration descriptor' }
$taskTotal = [BitConverter]::ToUInt16($taskConfigHeader,2)
$taskConfig = [CameraUsbDescriptors]::Descriptor($taskHub,$taskPort,2,$taskTotal)
if ($taskConfig.Length -ne $taskTotal) { throw 'Incomplete configuration descriptor' }
[IO.File]::WriteAllBytes((Join-Path $PWD "$OutputDirectory/configuration.bin"),$taskConfig)
$taskBosStatus='not queried'
try {
 $taskBosHeader=[CameraUsbDescriptors]::Descriptor($taskHub,$taskPort,15,5)
 if($taskBosHeader.Length -lt 5) { throw 'Truncated BOS header' }
 $taskBos=[CameraUsbDescriptors]::Descriptor($taskHub,$taskPort,15,[BitConverter]::ToUInt16($taskBosHeader,2))
 [IO.File]::WriteAllBytes((Join-Path $PWD "$OutputDirectory/bos.bin"),$taskBos)
 $taskBosStatus='retrieved'
} catch { $taskBosStatus=$_.Exception.Message }
@{ device=$taskInstance; parent=$taskParent; port=$taskPort; hub_path=$taskHub; speed_code=$taskConnection[23]; config_bytes=$taskTotal; bos_status=$taskBosStatus; note='Read-only USB hub descriptor queries; no vendor commands or device changes.' } | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'query.json')
Get-Content (Join-Path $OutputDirectory 'query.json')
