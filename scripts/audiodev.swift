import CoreAudio
import Foundation

// 列/切换 macOS 默认输出设备，用于验证系统音频→回环→采集的纯数字通路

func allDevices() -> [(AudioDeviceID, String, Bool)] {
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    let sys = AudioObjectID(kAudioObjectSystemObject)
    guard AudioObjectGetPropertyDataSize(sys, &addr, 0, nil, &size) == noErr else { return [] }
    var ids = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
    guard AudioObjectGetPropertyData(sys, &addr, 0, nil, &size, &ids) == noErr else { return [] }

    var defID = AudioDeviceID(0)
    var dAddr = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain)
    var dSize = UInt32(MemoryLayout<AudioDeviceID>.size)
    _ = AudioObjectGetPropertyData(sys, &dAddr, 0, nil, &dSize, &defID)

    return ids.compactMap { id in
        var nameAddr = AudioObjectPropertyAddress(
            mSelector: kAudioObjectPropertyName,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain)
        var cf: Unmanaged<CFString>?
        var s = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        let st = withUnsafeMutablePointer(to: &cf) { p in
            AudioObjectGetPropertyData(id, &nameAddr, 0, nil, &s, p)
        }
        guard st == noErr, let u = cf else { return nil }
        return (id, u.takeRetainedValue() as String, id == defID)
    }
}

func hasOutput(_ id: AudioDeviceID) -> Bool {
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyStreamConfiguration,
        mScope: kAudioDevicePropertyScopeOutput,
        mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(id, &addr, 0, nil, &size) == noErr, size > 0 else { return false }
    let buf = UnsafeMutableRawPointer.allocate(byteCount: Int(size), alignment: 16)
    defer { buf.deallocate() }
    guard AudioObjectGetPropertyData(id, &addr, 0, nil, &size, buf) == noErr else { return false }
    let list = buf.assumingMemoryBound(to: AudioBufferList.self)
    let n = Int(list.pointee.mNumberBuffers)
    return n > 0
}

let args = CommandLine.arguments
let devs = allDevices()

if args.count < 2 || args[1] == "list" {
    for (id, name, isDef) in devs where hasOutput(id) {
        print("\(isDef ? "*" : " ") \(id)\t\(name)")
    }
    exit(0)
}

if args[1] == "set", args.count >= 3 {
    guard let target = devs.first(where: { $0.1 == args[2] }) else {
        FileHandle.standardError.write("找不到设备：\(args[2])\n".data(using: .utf8)!)
        exit(1)
    }
    var dev = target.0
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain)
    let st = AudioObjectSetPropertyData(
        AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil,
        UInt32(MemoryLayout<AudioDeviceID>.size), &dev)
    print(st == noErr ? "已切换到：\(target.1)" : "切换失败（OSStatus \(st)）")
    exit(st == noErr ? 0 : 1)
}

FileHandle.standardError.write("用法：audiodev list | audiodev set <设备名>\n".data(using: .utf8)!)
exit(2)
