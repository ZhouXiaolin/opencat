# 进程树私有提交合计（字节）：分块渲染的子进程也算进门禁。
# 用法: powershell -NoProfile -ExecutionPolicy Bypass -File tree-mem.ps1 -Root <PID>
param([int]$Root)
$procs = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId
$ids = New-Object System.Collections.Generic.List[int]
$ids.Add($Root)
for ($i = 0; $i -lt $ids.Count; $i++) {
    foreach ($p in $procs) {
        if ($p.ParentProcessId -eq $ids[$i] -and $p.ProcessId -ne $ids[$i] -and -not $ids.Contains([int]$p.ProcessId)) {
            $ids.Add([int]$p.ProcessId)
        }
    }
}
$sum = [int64]0
foreach ($id in $ids) {
    $proc = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($proc) { $sum += $proc.PrivateMemorySize64 }
}
$sum
