// Synthetic inputs through Apple's real ImageIO HEVC encoder. No personal media.
import Foundation
import CoreGraphics
import ImageIO

guard CommandLine.arguments.count == 2 else { fatalError("Expected a new output directory") }
let directory = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
guard !FileManager.default.fileExists(atPath: directory.path) else { fatalError("Output already exists") }
try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
let width = 192, height = 128
func makeImage(_ space: CFString, _ depth: Int, _ alpha: Bool) -> CGImage {
    let maximum = depth == 16 ? 65535 : 255
    var data = Data()
    for y in 0..<height {
        for x in 0..<width {
            let a = alpha && x < width / 3 ? maximum / 3 : maximum
            let rgb: [Int] = y < height / 2
                ? (x < width / 2 ? [210, 45, 35] : [45, 185, 65])
                : (x < width / 2 ? [40, 70, 205] : [220, 175, 55])
            for value in rgb + [255] {
                let v = value == 255 ? a : value * a / 255
                if depth == 16 { var n = UInt16(v).littleEndian; withUnsafeBytes(of: &n) { data.append(contentsOf: $0) } }
                else { data.append(UInt8(v)) }
            }
        }
    }
    let provider = CGDataProvider(data: data as CFData)!
    let flags = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
        .union(depth == 16 ? .byteOrder16Little : CGBitmapInfo(rawValue: 0))
    return CGImage(width: width, height: height, bitsPerComponent: depth,
        bitsPerPixel: depth * 4, bytesPerRow: width * depth / 8 * 4,
        space: CGColorSpace(name: space)!, bitmapInfo: flags,
        provider: provider, decode: nil, shouldInterpolate: false, intent: .relativeColorimetric)!
}
var cases = [[String: Any]]()
func encode(_ name: String, _ space: CFString, _ depth: Int = 8, _ alpha: Bool = false, _ orientation: Int = 1) {
    let url = directory.appendingPathComponent(name)
    let dest = CGImageDestinationCreateWithURL(url as CFURL, "public.heic" as CFString, 1, nil)!
    let options: [CFString: Any] = [kCGImageDestinationLossyCompressionQuality: 1.0,
        kCGImagePropertyOrientation: orientation, kCGImageDestinationEmbedThumbnail: false]
    CGImageDestinationAddImage(dest, makeImage(space, depth, alpha), options as CFDictionary)
    let ok = CGImageDestinationFinalize(dest)
    cases.append(["file": name, "generated": ok, "sourceBits": depth,
        "sourceColorSpace": space as String, "sourceAlpha": alpha, "orientation": orientation,
        "expectedDimensions": orientation >= 5 ? [height, width] : [width, height],
        "expect": name.contains("pq") || name.contains("hlg") ? "reject-hdr" : name.contains("sdr-16bit") ? "reject-ambiguous" : "accept"])
    print("\(name): \(ok)")
}
for orientation in 1...8 { encode("apple-orientation-\(orientation).heic", CGColorSpace.sRGB, 8, false, orientation) }
encode("apple-display-p3.heic", CGColorSpace.displayP3)
encode("apple-alpha.heic", CGColorSpace.sRGB, 8, true)
encode("apple-sdr-16bit-source.heic", CGColorSpace.sRGB, 16)
encode("apple-pq-16bit-source.heic", CGColorSpace.itur_2100_PQ, 16)
encode("apple-hlg-16bit-source.heic", CGColorSpace.itur_2100_HLG, 16)
// ImageIO writes unspecified primaries/transfer for the 16-bit sRGB source on
// this SDK. Make a separately identified metadata fixture with explicit sRGB.
var explicit = try Data(contentsOf: directory.appendingPathComponent("apple-sdr-16bit-source.heic"))
if let range = explicit.range(of: Data("nclx".utf8)) {
    let n = range.lowerBound
    explicit.replaceSubrange((n+4)..<(n+8), with: [0,1,0,13])
    try explicit.write(to: directory.appendingPathComponent("apple-10bit-explicit-srgb.heic"))
    cases.append(["file":"apple-10bit-explicit-srgb.heic","generated":true,"sourceBits":16,
        "sourceColorSpace":"sRGB metadata derived from the Apple-encoded source", "sourceAlpha":false,
        "orientation":1,"expectedDimensions":[width,height],"expect":"accept"])
}
try JSONSerialization.data(withJSONObject: ["generator": "Apple ImageIO; synthetic color quadrants", "cases": cases], options: [.prettyPrinted, .sortedKeys])
    .write(to: directory.appendingPathComponent("manifest.json"))
