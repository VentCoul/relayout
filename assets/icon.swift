// Renders the app icon and the menu bar (template) icon. Usage: swift icon.swift <out-dir>
import AppKit
func render(_ size: CGFloat, _ file: String, template: Bool) {
  let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size), pixelsHigh: Int(size), bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
  NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
  let r = NSRect(x: 0, y: 0, width: size, height: size)
  var fg = NSColor.black
  if !template {
    let inset = r.insetBy(dx: size * 0.08, dy: size * 0.08)
    let path = NSBezierPath(roundedRect: inset, xRadius: size * 0.2, yRadius: size * 0.2)
    NSGradient(starting: NSColor(red: 0.16, green: 0.38, blue: 0.95, alpha: 1), ending: NSColor(red: 0.05, green: 0.17, blue: 0.55, alpha: 1))!.draw(in: path, angle: -90)
    fg = .white
  }
  let text = "Aя" as NSString
  let font = NSFont.systemFont(ofSize: size * (template ? 0.62 : 0.42), weight: .bold)
  let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: fg]
  let s = text.size(withAttributes: attrs)
  text.draw(at: NSPoint(x: (size - s.width) / 2, y: (size - s.height) / 2), withAttributes: attrs)
  NSGraphicsContext.current = nil
  try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: file))
}
let out = CommandLine.arguments[1]
render(1024, out + "/icon.png", template: false)
render(44, out + "/tray.png", template: true)
