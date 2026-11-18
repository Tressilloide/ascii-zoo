// Solo per la prova automatica su macOS (job "macos" della CI): simula un clic sinistro
// nel punto (x, y) in coordinate di CoreGraphics (punti, origine in alto a sinistra dello
// schermo principale), come un clic vero del mouse.
// Uso: swiftc -O -o clic click.swift && ./clic 400 400
// Scrive se il processo ha il permesso Accessibilità: senza, macOS può scartare i clic simulati.
import ApplicationServices
import CoreGraphics
import Foundation

let a = CommandLine.arguments
guard a.count == 3, let x = Double(a[1]), let y = Double(a[2]) else {
    print("uso: clic X Y")
    exit(2)
}
let p = CGPoint(x: x, y: y)
let fonte = CGEventSource(stateID: .hidSystemState)
func manda(_ tipo: CGEventType) {
    CGEvent(mouseEventSource: fonte, mouseType: tipo, mouseCursorPosition: p, mouseButton: .left)?
        .post(tap: .cghidEventTap)
}
manda(.mouseMoved)
usleep(300_000)
manda(.leftMouseDown)
usleep(80_000)
manda(.leftMouseUp)
usleep(300_000)
print("clic simulato in \(Int(x)),\(Int(y)); permesso Accessibilità: \(AXIsProcessTrusted())")
