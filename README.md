# imgtool-rs

一个基于 Rust 的图片工具 CLI，支持：
- 元数据读取（含常见 EXIF 字段）
- 按比例缩放
- 压缩文件体积（保持分辨率不变）
- 格式转换
- 目录级批处理流水线（resize/compress 组合）

## 构建

```bash
cargo build --release
```

## 快速查看帮助

```bash
cargo run -- --help
cargo run -- meta --help
cargo run -- resize --help
cargo run -- compress --help
cargo run -- convert --help
cargo run -- batch --help
```

## 支持格式

- 元数据读取：
  - 栅格：`jpg/jpeg/png/webp/tiff/tif/bmp/gif`
  - RAW：`arw/sr2/srf/nef/nrw/cr2/cr3/crw/dng`（基于 EXIF 能力）
- `resize` 输入/输出：`jpg/jpeg/png/webp/tiff/tif/bmp/gif`
- `compress` 输出：`jpg/jpeg/png`
- `convert` 输出：`jpg/jpeg/png/webp/bmp/gif/tiff/tif`

## 命令说明

### 1) 元数据读取 `meta`

```bash
imgtool-rs meta <PATH> [--json] [--no-gps] [--include-hidden] [--fail-fast]
```

示例：

```bash
imgtool-rs meta ./photo.jpg
imgtool-rs meta ./photos --json
imgtool-rs meta ./photos --json --include-hidden --fail-fast
```

说明：
- 仅处理合法图片扩展名。
- 解析失败项不会输出详情记录，只在最终统计里计入失败数。

### 2) 按比例缩放 `resize`

```bash
imgtool-rs resize <INPUT> <OUTPUT> --scale <RATIO> [--filter <FILTER>]
```

- `--filter` 可选：`nearest/triangle/catmullrom/gaussian/lanczos3`
- 默认 filter：`lanczos3`

示例：

```bash
imgtool-rs resize in.jpg out.jpg --scale 0.5
imgtool-rs resize in.png out.png --scale 2.0 --filter catmullrom
```

### 3) 压缩体积 `compress`

```bash
imgtool-rs compress <INPUT> <OUTPUT> [--quality <1..100>] [--max-size-kb <KB>]
```

示例：

```bash
imgtool-rs compress in.jpg out.jpg --quality 70
imgtool-rs compress in.jpg out.jpg --quality 95 --max-size-kb 200
imgtool-rs compress in.png out.png
```

说明：
- 保持分辨率不变，只做重编码。
- `--max-size-kb` 目前仅支持 `jpg/jpeg` 输出（内部会自动搜索质量）。

### 4) 格式转换 `convert`

```bash
imgtool-rs convert <INPUT> <OUTPUT> [--quality <1..100>]
```

示例：

```bash
imgtool-rs convert in.png out.jpg --quality 80
imgtool-rs convert in.jpg out.webp
imgtool-rs convert in.jpg out.tiff
```

说明：
- 保持分辨率不变。
- `--quality` 仅对 JPEG 输出生效。

### 5) 批处理流水线 `batch`

```bash
imgtool-rs batch <INPUT_DIR> <OUTPUT_DIR> [--resize-scale <RATIO>] [--resize-filter <FILTER>] [--compress] [--quality <1..100>] [--max-size-kb <KB>] [--include-hidden] [--fail-fast]
```

示例：

```bash
imgtool-rs batch ./in ./out --resize-scale 0.5
imgtool-rs batch ./in ./out --compress --quality 70
imgtool-rs batch ./in ./out --resize-scale 0.5 --compress --max-size-kb 300
imgtool-rs batch ./in ./out --compress --fail-fast
```

说明：
- 自动保留输入目录结构到输出目录。
- 至少需要一种操作：`--resize-scale` 或压缩相关参数（`--compress/--quality/--max-size-kb`）。
- 批处理默认继续执行并统计失败；`--fail-fast` 会遇错即停。

## 测试

```bash
cargo test
```

