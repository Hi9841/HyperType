/** Read the Windows display scale (e.g. 1.25 at 125%). */
import { runText } from "./node-utils.ts";

export function windowsDisplayScale(): number {
  const ps = `
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ParityDpi {
  [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr hwnd);
  [DllImport("gdi32.dll")] static extern int GetDeviceCaps(IntPtr hdc, int nIndex);
  [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr hwnd, IntPtr hdc);
  public static double Scale() {
    SetProcessDPIAware();
    IntPtr hdc = GetDC(IntPtr.Zero);
    int dpi = GetDeviceCaps(hdc, 88);
    ReleaseDC(IntPtr.Zero, hdc);
    return dpi / 96.0;
  }
}
"@
[ParityDpi]::Scale()
`;
  const { exitCode, stdout, stderr } = runText("powershell", [
    "-NoProfile",
    "-Command",
    ps,
  ]);
  const text = stdout.trim();
  const scale = Number.parseFloat(text);
  if (!Number.isFinite(scale) || scale <= 0) {
    throw new Error(
      `Could not read display scale (exit ${exitCode}). stdout=${text} stderr=${stderr}`,
    );
  }
  return scale;
}
