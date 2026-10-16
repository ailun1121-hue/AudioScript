#!/usr/bin/env bash
# 清理 DMG 打包失败后残留的临时卷与映像：`bundle_dmg.sh` 的 hdiutil detach 步骤偶发失败，残留卷只增不减，一次失败会变成永久失败（实测连跑十几次攒了 8 个卷）。
# 用法：pnpm clean:dmg；更根本的规避是 pnpm build:app（只打 .app，跳过 DMG）。
# 只清自己的构建产物：`target/**/rw.*.dmg` 与 `/Volumes/dmg.*`，不碰用户数据。

set -uo pipefail

echo "── 卸载残留的临时卷 ──────────────────────────────"

# 用 `mount` 而非 `hdiutil info` 的输出：前者格式稳定，且 `-s1` 那一级就是要卸载的卷。
found=0
while read -r dev _ _ _ mp _; do
  [ -n "${mp:-}" ] || continue
  case "$mp" in
    /Volumes/dmg.*) ;;
    *) continue ;;
  esac
  found=1
  if hdiutil detach "$dev" >/dev/null 2>&1; then
    echo "  ✓ 已卸载 $dev  ($mp)"
  elif hdiutil detach "$dev" -force >/dev/null 2>&1; then
    # 强制卸载对临时构建映像安全（里面只有刚拷进去的 .app 副本），是最后手段，故打出来留痕
    echo "  ✓ 已强制卸载 $dev  ($mp)"
  else
    echo "  ✗ 卸载失败 $dev  ($mp)  ← 可能有程序正打开它，关掉再试" >&2
  fi
done < <(mount | grep -E ' on /Volumes/dmg\.')

[ "$found" -eq 1 ] || echo "  （没有需要卸载的）"

echo
echo "── 删除残留的临时映像 ────────────────────────────"

root="$(cd "$(dirname "$0")/.." && pwd)"
count=$(find "$root/src-tauri/target" -name 'rw.*.dmg' 2>/dev/null | wc -l | tr -d ' ')

if [ "$count" = "0" ]; then
  echo "  （没有需要删除的）"
else
  size=$(find "$root/src-tauri/target" -name 'rw.*.dmg' -exec du -sk {} + 2>/dev/null |
    awk '{s+=$1} END {printf "%.0f", s/1024}')
  find "$root/src-tauri/target" -name 'rw.*.dmg' -delete 2>/dev/null
  echo "  ✓ 删除 $count 个临时映像，回收约 ${size} MB"
fi

echo
echo "完成。可以重新执行 pnpm tauri build。"
