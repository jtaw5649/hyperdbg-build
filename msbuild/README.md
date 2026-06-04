# MSBuild Overlay Files

Source templates for Wine/MSBuild workarounds. Generated copies live under
`hyperdbg-build/out/msbuild/`.

## Files

- `Directory.Build.targets.template`: disables the `script-engine` post-build
  event under Wine/MSBuild.
- `masm-wine.targets`: skips the MASM XamlTaskFactory path and links
  preassembled MASM objects.
