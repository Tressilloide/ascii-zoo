// Solo per la prova automatica su macOS (job "macos" della CI): carica il modulo
// "ASCII Zoo.saver" come fa macOS (NSBundle e la sua NSPrincipalClass), crea nello stesso
// processo una vista grande (come uno schermo) e un'anteprima piccola, le anima per qualche
// secondo e salva un PNG di ognuna nella cartella indicata.
// Esce con 1 se il modulo non si carica, se la classe non è una ScreenSaverView o se
// un'immagine è vuota (solo il colore di fondo).
// Con --willstop alla fine manda la notifica "com.apple.screensaver.willstop": da macOS 14
// il modulo deve chiudere il processo con exit(0); se non lo fa, esce con 3.
// Uso: swiftc -O -o prova-saver saver.swift && ./prova-saver "ASCII Zoo.saver" cartella [--willstop]
import AppKit
import ScreenSaver

setvbuf(stdout, nil, _IOLBF, 0)
let a = CommandLine.arguments
guard a.count >= 3 else {
    print("uso: prova-saver MODULO.saver CARTELLA [--willstop]")
    exit(2)
}
let percorso = a[1]
let cartella = URL(fileURLWithPath: a[2], isDirectory: true)
try? FileManager.default.createDirectory(at: cartella, withIntermediateDirectories: true)

guard let bundle = Bundle(path: percorso) else {
    print("::error::\(percorso) non è un bundle")
    exit(1)
}
do {
    try bundle.loadAndReturnError()
} catch {
    print("::error::il modulo non si carica: \(error)")
    exit(1)
}
guard let classe = bundle.principalClass as? ScreenSaverView.Type else {
    print("::error::la NSPrincipalClass non è una ScreenSaverView: \(String(describing: bundle.principalClass))")
    exit(1)
}
let versione = bundle.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?"
print("modulo caricato: \(bundle.bundleIdentifier ?? "?") \(versione), classe \(NSStringFromClass(classe))")

guard let schermo = classe.init(frame: NSRect(x: 0, y: 0, width: 1024, height: 768), isPreview: false),
      let anteprima = classe.init(frame: NSRect(x: 0, y: 0, width: 296, height: 184), isPreview: true)
else {
    print("::error::initWithFrame:isPreview: ha restituito nil")
    exit(1)
}
print("hasConfigureSheet: \(schermo.hasConfigureSheet), animationTimeInterval: \(schermo.animationTimeInterval)")

// la scena parte dal nero e si accende in 1,2 secondi: si anima per 3 secondi veri
schermo.startAnimation()
anteprima.startAnimation()
var fotogrammi = 0
var speso = 0.0
let fine = Date().addingTimeInterval(3.0)
while Date() < fine {
    let t0 = Date()
    schermo.animateOneFrame()
    speso += Date().timeIntervalSince(t0)
    anteprima.animateOneFrame()
    fotogrammi += 1
    RunLoop.main.run(until: Date().addingTimeInterval(1.0 / 30.0))
}
print("\(fotogrammi) giri in 3 s; animateOneFrame della vista grande: \(String(format: "%.1f", speso * 1000 / Double(fotogrammi))) ms in media")

/// fotografa la vista (passa da drawRect:, come la cache di AppKit), salva il PNG e conta
/// i pixel accesi: più chiari del fondo delle scene (6, 8, 12)
func fotografa(_ v: ScreenSaverView, _ nome: String) -> Bool {
    let t0 = Date()
    guard let rep = v.bitmapImageRepForCachingDisplay(in: v.bounds) else {
        print("::error::\(nome): niente bitmap")
        return false
    }
    v.cacheDisplay(in: v.bounds, to: rep)
    let ms = Date().timeIntervalSince(t0) * 1000
    if let png = rep.representation(using: .png, properties: [:]) {
        try? png.write(to: cartella.appendingPathComponent("\(nome).png"))
    }
    guard let img = rep.cgImage else {
        print("::error::\(nome): niente CGImage")
        return false
    }
    // i pixel in un formato noto: RGBA, 8 bit
    let (w, h) = (img.width, img.height)
    var dati = [UInt8](repeating: 0, count: w * h * 4)
    let ok = dati.withUnsafeMutableBytes { p -> Bool in
        guard let ctx = CGContext(data: p.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
        return true
    }
    guard ok else {
        print("::error::\(nome): non si leggono i pixel")
        return false
    }
    var accesi = 0
    for i in stride(from: 0, to: dati.count, by: 4) where max(dati[i], dati[i + 1], dati[i + 2]) > 60 {
        accesi += 1
    }
    let quota = Double(accesi) * 100 / Double(w * h)
    print("\(nome): \(w)x\(h) pixel, disegnata in \(String(format: "%.1f", ms)) ms, accesi \(accesi) (\(String(format: "%.2f", quota))%)")
    if accesi < 20 {
        print("::error::\(nome): l'immagine è vuota (solo il fondo)")
        return false
    }
    if quota > 90 {
        print("::error::\(nome): quasi tutti i pixel accesi, non sembra una scena")
        return false
    }
    return true
}

let tutto = fotografa(schermo, "schermo") && fotografa(anteprima, "anteprima")
if !tutto {
    exit(1)
}

if a.contains("--willstop") {
    let os = ProcessInfo.processInfo.operatingSystemVersion
    print("macOS \(os.majorVersion).\(os.minorVersion): mando com.apple.screensaver.willstop")
    DistributedNotificationCenter.default().postNotificationName(
        NSNotification.Name("com.apple.screensaver.willstop"), object: nil, userInfo: nil, deliverImmediately: true)
    // il modulo esce dopo 2 secondi: qui se ne aspettano 8
    RunLoop.main.run(until: Date().addingTimeInterval(8))
    if os.majorVersion >= 14 {
        print("::error::dopo com.apple.screensaver.willstop il processo è ancora vivo")
        exit(3)
    }
    print("prima di macOS 14 il modulo non esce da solo: va bene così")
}
schermo.stopAnimation()
anteprima.stopAnimation()
exit(0)
