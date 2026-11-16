# Trampoline forwarding all parameters to test.ps1
& "$PSScriptRoot\test.ps1" @args
exit $LASTEXITCODE
