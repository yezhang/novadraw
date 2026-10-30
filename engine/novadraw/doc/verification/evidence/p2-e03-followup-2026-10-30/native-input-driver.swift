import Foundation
import CoreGraphics

let args = CommandLine.arguments
func mouse(_ type: CGEventType, _ x: Double, _ y: Double) {
    CGEvent(mouseEventSource: nil, mouseType: type,
            mouseCursorPosition: CGPoint(x: x, y: y), mouseButton: .left)!.post(tap: .cghidEventTap)
}
if args[1] == "key" {
    let code = CGKeyCode(args[2])!
    CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: true)!.post(tap: .cghidEventTap)
    CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: false)!.post(tap: .cghidEventTap)
} else {
    let x = Double(args[2])!, y = Double(args[3])!
    mouse(.mouseMoved, x, y)
    Thread.sleep(forTimeInterval: 0.1)
    mouse(.leftMouseDown, x, y)
    let endX = Double(args[4])!, endY = Double(args[5])!
    for index in 1...12 {
        Thread.sleep(forTimeInterval: 0.04)
        mouse(.leftMouseDragged, x + (endX-x)*Double(index)/12, y + (endY-y)*Double(index)/12)
    }
    if args[1] == "cancel" {
        CGEvent(keyboardEventSource:nil,virtualKey:53,keyDown:true)!.post(tap:.cghidEventTap)
        CGEvent(keyboardEventSource:nil,virtualKey:53,keyDown:false)!.post(tap:.cghidEventTap)
        Thread.sleep(forTimeInterval: 0.1)
    }
    mouse(.leftMouseUp, endX, endY)
}
