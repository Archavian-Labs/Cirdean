param([string]$Device = 'UGREEN Camera', [string]$Control = '', [int]$Value = 0, [int]$Flags = 2)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
[ComImport, Guid("29840822-5B84-11D0-BD3B-00A0C911CE86"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface ICreateDevEnum { [PreserveSig] int CreateClassEnumerator(ref Guid category, out IEnumMoniker enumerator, int flags); }
[ComImport, Guid("55272A00-42CB-11CE-8135-00AA004BB851"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IPropertyBag { [PreserveSig] int Read([MarshalAs(UnmanagedType.LPWStr)] string name, [MarshalAs(UnmanagedType.Struct)] out object value, IntPtr log); [PreserveSig] int Write([MarshalAs(UnmanagedType.LPWStr)] string name, ref object value); }
[ComImport, Guid("C6E13370-30AC-11d0-A18C-00A0C9118956"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IAMCameraControl { [PreserveSig] int GetRange(int property,out int min,out int max,out int step,out int def,out int caps); [PreserveSig] int Set(int property,int value,int flags); [PreserveSig] int Get(int property,out int value,out int flags); }
[ComImport, Guid("C6E13360-30AC-11d0-A18C-00A0C9118956"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IAMVideoProcAmp { [PreserveSig] int GetRange(int property,out int min,out int max,out int step,out int def,out int caps); [PreserveSig] int Set(int property,int value,int flags); [PreserveSig] int Get(int property,out int value,out int flags); }
public class CameraSetting { public string Name; public bool Supported; public int Min,Max,Step,Default,Capabilities,Value,Flags,ReadStatus; }
public sealed class CameraControls : IDisposable {
 object filter;
 string[] cameraNames={"Pan","Tilt","Roll","Zoom","Exposure","Iris","Focus"};
 string[] videoNames={"Brightness","Contrast","Hue","Saturation","Sharpness","Gamma","ColorEnable","WhiteBalance","BacklightCompensation","Gain"};
 public CameraControls(string device) {
  var creator=(ICreateDevEnum)Activator.CreateInstance(Type.GetTypeFromCLSID(new Guid("62BE5D10-60EB-11d0-BD3B-00A0C911CE86")));
  IEnumMoniker enumerator=null;
  try {
   Guid category=new Guid("860BB310-5D01-11d0-BD3B-00A0C911CE86");
   if(creator.CreateClassEnumerator(ref category,out enumerator,0)!=0) throw new Exception("No cameras");
   var monikers=new IMoniker[1];
   while(enumerator.Next(1,monikers,IntPtr.Zero)==0) {
    object bag=null;
    try {
     Guid bagId=typeof(IPropertyBag).GUID;
     monikers[0].BindToStorage(null,null,ref bagId,out bag);
     object name; ((IPropertyBag)bag).Read("FriendlyName",out name,IntPtr.Zero);
     if((string)name==device) {
      Guid filterId=new Guid("56a86895-0ad4-11ce-b03a-0020af0ba770");
      monikers[0].BindToObject(null,null,ref filterId,out filter); break;
     }
    } finally { if(bag!=null) Marshal.ReleaseComObject(bag); Marshal.ReleaseComObject(monikers[0]); }
   }
   if(filter==null) throw new Exception("Camera not found: "+device);
  } finally { if(enumerator!=null) Marshal.ReleaseComObject(enumerator); Marshal.ReleaseComObject(creator); }
 }
 public List<CameraSetting> Read() {
  var result=new List<CameraSetting>();
  for(int group=0;group<2;group++) {
   var names=group==0?cameraNames:videoNames;
   for(int i=0;i<names.Length;i++) {
    var s=new CameraSetting{Name=names[i]}; int status;
    try {
     if(group==0) { var api=(IAMCameraControl)filter; status=api.GetRange(i,out s.Min,out s.Max,out s.Step,out s.Default,out s.Capabilities); s.ReadStatus=api.Get(i,out s.Value,out s.Flags); }
     else { var api=(IAMVideoProcAmp)filter; status=api.GetRange(i,out s.Min,out s.Max,out s.Step,out s.Default,out s.Capabilities); s.ReadStatus=api.Get(i,out s.Value,out s.Flags); }
     s.Supported=status==0;
    } catch(InvalidCastException) { s.Supported=false; }
    result.Add(s);
   }
  } return result;
 }
 public void Set(string name,int value,int flags) {
  var setting=Read().Find(s=>s.Name==name);
  if(setting==null||!setting.Supported) throw new Exception("Unsupported control "+name);
  if(value<setting.Min||value>setting.Max||(setting.Step>0&&(value-setting.Min)%setting.Step!=0)) throw new Exception("Value out of range or step");
  if((setting.Capabilities&flags)!=flags) throw new Exception("Unsupported auto/manual flags");
  int index=Array.IndexOf(cameraNames,name);
  int hr=index>=0?((IAMCameraControl)filter).Set(index,value,flags):((IAMVideoProcAmp)filter).Set(Array.IndexOf(videoNames,name),value,flags);
  Marshal.ThrowExceptionForHR(hr);
 }
 public void Dispose() { if(filter!=null) { Marshal.ReleaseComObject(filter); filter=null; } }
}
"@
$taskCamera = [CameraControls]::new($Device)
try {
    if ($Control) { $taskCamera.Set($Control, $Value, $Flags) }
    $taskCamera.Read() | ConvertTo-Json -Depth 3
} finally { $taskCamera.Dispose() }
