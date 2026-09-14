param([int]$ProbeProcessId)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Security.Principal;
public static class ProbeToken {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int id);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool GetTokenInformation(IntPtr token,int kind,IntPtr data,int size,out int required);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 public static object Read(int id) {
  IntPtr process=OpenProcess(0x1000,false,id), token=IntPtr.Zero;
  if(process==IntPtr.Zero) throw new System.ComponentModel.Win32Exception();
  try {
   if(!OpenProcessToken(process,8,out token)) throw new System.ComponentModel.Win32Exception();
   string user=null,integrity=null; int? elevated=null,type=null;
   foreach(int kind in new[]{1,18,20,25}) {
    GetTokenInformation(token,kind,IntPtr.Zero,0,out int size);
    IntPtr data=Marshal.AllocHGlobal(size);
    try {
     if(!GetTokenInformation(token,kind,data,size,out size)) throw new System.ComponentModel.Win32Exception();
     if(kind==1) user=new SecurityIdentifier(Marshal.ReadIntPtr(data)).Value;
     if(kind==25) integrity=new SecurityIdentifier(Marshal.ReadIntPtr(data)).Value;
     if(kind==18) type=Marshal.ReadInt32(data);
     if(kind==20) elevated=Marshal.ReadInt32(data);
    } finally { Marshal.FreeHGlobal(data); }
   }
   return new {user_sid=user,integrity_sid=integrity,elevated=elevated,elevation_type=type,process_id=id};
  } finally { if(token!=IntPtr.Zero)CloseHandle(token); CloseHandle(process); }
 }
}
'@
$taskPolicies = @{}
foreach ($taskSpec in @(
    @('HKLM:\SOFTWARE\Policies\Microsoft\Camera', 'AllowCamera'),
    @('HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppPrivacy', 'LetAppsAccessCamera'),
    @('HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppPrivacy', 'LetAppsAccessCamera_ForceAllowTheseApps'),
    @('HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppPrivacy', 'LetAppsAccessCamera_ForceDenyTheseApps'),
    @('HKCU:\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam', 'Value')
)) {
    $taskProperty = Get-ItemProperty -LiteralPath $taskSpec[0] -Name $taskSpec[1] -ErrorAction SilentlyContinue
    $taskPolicies[($taskSpec -join '\')] = if ($null -eq $taskProperty) { $null } else { $taskProperty.($taskSpec[1]) }
}
$taskOS = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
@{ token=[ProbeToken]::Read($ProbeProcessId); os=@{build=$taskOS.CurrentBuildNumber;ubr=$taskOS.UBR;display_version=$taskOS.DisplayVersion};policy_values=$taskPolicies;note='Read-only. Null policy means absent/unreadable, not permission denied. ConsentStore is diagnostic evidence, not a documented guarantee of per-process access.' } | ConvertTo-Json -Depth 8
