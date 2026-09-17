# 跨发行版兼容、坑点与测试方案

## 发行版差异

| 点 | Debian/Ubuntu | Fedora/RHEL | Arch | 容器 / 部分云主机 |
| --- | --- | --- | --- | --- |
| DMI sysfs | 有（裸机/KVM） | 同左 | 同左 | 经常整棵 `/sys/class/dmi` 不存在 |
| `product_serial` 权限 | udev 常设 0400 | 同左 | 同左 | — |
| `pci.ids` | 包 `pci.ids` 或 `hwdata`，路径 `/usr/share/misc/pci.ids` | `hwdata`，`/usr/share/hwdata/pci.ids` | `hwdata` | 可能无 |
| `usb.ids` | 常随 `usbutils`/`hwdata` | `/usr/share/misc/usb.ids` 或 `hwdata` | 同 pci.ids | 可能无 |
| `/proc/bus/input/devices` | 桌面有 | 同 | 同 | 无头/容器常空 |
| NUMA sysfs | 多路服务器有 nodeN | 同 | 同 | 单节点或未启用 NUMA 只有 node0 |
| hwmon 驱动 | 需 `linux-modules-extra` 或自己加载 coretemp/k10temp | 内核包较全 | 较全 | 虚拟机常无 |
| NVMe 节点权限 | `root:disk` 0660 | 同左 | 同左 | 无 NVMe 时走 virtio `vd*` |
| 桌面 | GNOME 下 `pkexec` 保 DISPLAY 比裸 `sudo` 稳 | 同 | 同 | 无 GUI |
| DRM | `amdgpu`/`i915` 节点较稳 | 同，另有 `xe` | 同 | 常无 `/sys/class/drm` |
| NVIDIA procfs | 专有驱动才有 `/proc/driver/nvidia` | 同 | 同 | 云主机几乎没有 |

ARM 板子：`/proc/cpuinfo` 没有 `model name` / `physical id`，只有 `CPU part` 等。CPU probe 已按键名解析，缺键就 `not_found`，不要按 x86 行号切。

## 已知坑

1. **容器看不到主机 SMBIOS/hwmon。** 这不是 bug。提示里写了路径。若要在容器里测真实硬件，需要 `--privileged` 且挂载对应 sysfs，生产工具仍应在主机跑。
2. **`cpu MHz` 在虚拟机里是恒定值**，且常常没有 `cpufreq`。界面会退回 cpuinfo 频率并加 note。
3. **virtio 块设备 `queue/rotational` 经常是 1**，不能据此判断“这是机械硬盘”。类型列优先看名字（`vd*` / `nvme*`）。
4. **PCI class `0xffff00`** 出现在部分 virtio 设备上，内核用兜底类码。名称解析只能靠 vendor/device + pci.ids。
5. **NVMe ioctl 结构体大小必须是 72 字节（64-bit）**。搞错 `_IOWR` 会 `ENOTTY`。本仓库用 `assert!(size_of::<NvmeAdminCmd>() == 72)`。
6. **不要把 SMART 的 Kelvin 当成摄氏度。** 规范是绝对温度，代码减 273.15。
7. **HTML 导出必须转义 DMI 字符串。** 厂商自定义字段可能含 `<`。
8. **AppImage + musl + egui/glow 基本不现实。** CLI musl、GUI glibc+linuxdeploy。
9. **sudo 掉 DISPLAY。** GUI 提权用 `pkexec` 或 `sudo -E`，并检查 `xhost`。
10. **`/sys/class/nvme/nvme0n1` 是命名空间不是控制器。** 枚举要过滤 `nvme\d+n\d+`。
11. **NVIDIA 占用率不在公开 sysfs。** 不要用 `nvidia-smi` 填这个洞；界面标 `unsupported`。
12. **`O_DIRECT` 在 tmpfs 上基本必失败**（EINVAL）。测试文件放当前目录或 ext4 数据盘。缓冲必须 512/4K 对齐，用 `posix_memalign`，不要 `Vec<u8>`。
13. **pkexec 与 polkitd 不是同一个包。** 只有 `polkitd` 时 `aida elevate` 会落到 sudo，无 TTY 的 GUI 按钮会失败。
14. **AppImage 在无 FUSE 容器里** 要用 `APPIMAGE_EXTRACT_AND_RUN=1`，脚本已默认导出该变量。
15. **virtio / docker0 的 `speed` 经常是 -1。** 不是解析失败，标 `unsupported`。
16. **不要调用 `ip`/`ifconfig`/`lsusb`。** 地址用 `getifaddrs`；USB 枚举 `/sys/bus/usb/devices`，接口节点名含 `:` 要跳过。
17. **`/proc/bus/input/devices` 在无头虚拟机里经常不存在。** 提示路径即可。
18. **NUMA `meminfo` 行是 `Node 0 MemTotal:`**，不能当 `/proc/meminfo` 的 `MemTotal:` 去切。
19. **告警日志只在状态变化时写一行 JSONL。** 持续越限不会刷盘；恢复再写 `ok`。
20. **`/proc/stat` 的 `cpu` 行与 `cpu0` 行不要混用。** 整机利用率用前者，每核用后者。
21. **`/proc/diskstats` 的扇区按 512 字节计**（即使盘是 4K）。差分前先乘 512。
22. **Secure Boot efivar 前 4 字节是属性。** 真正的开关是第 5 个字节。
23. **无 `power_supply` 不是 bug。** 多数服务器/容器没有电池节点。

## 测试方案

### 自动化（本仓库）

```bash
cargo test --no-default-features
```

- 夹具：伪造 hwmon、cpuinfo、pci.ids、os-release（不依赖本机硬件）
- 权限：临时文件 `chmod 000`，非 root 断言 `permission_denied`（root 环境会跳过该断言）
- 现场：`tests/live_collect.rs` 对真实 `/proc` `/sys` 采集一次，只要求不 panic，允许大量 `not_found`
- 基准：`BenchRequest::quick()` 小缓冲，避免 CI 超时

### 手工矩阵（发布前）

| 环境 | 看什么 |
| --- | --- |
| x86_64 裸机 Fedora/Ubuntu 普通用户 | CPU/PCI 有值；DMI serial 为权限不足 |
| 同上 root | serial/UUID/SMART 有值 |
| AMD + k10temp、Intel + coretemp | 温度折线 |
| NVMe 笔记本 | sysfs 型号 + SMART percentage_used |
| QEMU virtio 虚拟机 | 无 hwmon/DMI/GPU 时提示正确；块设备为 `vd*`；O_DIRECT 看文件系统 |
| AMD amdgpu / Intel i915 | GPU 页有 busy 或 gt 频率 |
| NVIDIA 专有驱动 | 有 model/vbios；busy 为 unsupported |
| aarch64 树莓派/ARM 云 | cpuinfo 缺字段不崩 |
| Wayland + X11 各测一次 GUI | 字体、折线、导出按钮 |

### 回归命令

```bash
# 对比普通用户 vs root 的 access 统计
aida collect > /tmp/u.json
sudo aida collect > /tmp/r.json
# 抽查 product_serial.access / nvme.smart.access
```

不要用“字段是否为空”当回归标准，要用 `access` 枚举：root 下 `permission_denied` 才是回归失败。
