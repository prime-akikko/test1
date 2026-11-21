# AGV QR Floor Localization — Shopping Shortlist

Concrete **camera + compute** picks for the design in [agv-qr-floor-localization.md](./agv-qr-floor-localization.md).  
Target: downward global-shutter camera, ~1 cm planar accuracy, USB3 (GigE alternate noted), Jetson or x86 IPC — not phone/web.

Prices are **approximate street/list bands** from distributor listings (USD / EUR where seen). Availability and list prices change; treat as order-of-magnitude for BOM planning, not a quote.

---

## How to read this list

| Role | Pick |
| --- | --- |
| **Budget / lab** | Option A |
| **Industrial / pilot lane** | Option C |
| Middle path (cheap industrial camera + familiar x86) | Option B |

All three assume: mono global shutter, C/CS-mount fixed lens matched to ~150–250 mm working height, optional ring light + polarizer later.

**Lens rule of thumb (all options):** at \(h \approx 200\) mm over floor with a ~1/2″–2/3″ sensor, start with **6 mm or 8 mm** fixed C-mount (validate FOV so ≥1 marker of 80–150 mm side fills ≥40–60 px). Lock focus/iris after calibration.

---

## Option A — Budget / lab (Basler + Jetson)

**Best for:** bench calibration, first AGV mount, software bring-up, mock marker grid.

| Piece | SKU / product | Why |
| --- | --- | --- |
| **Camera** | **Basler ace 2 Basic `a2A1920-160umBAS`** — USB3 Vision, mono, Sony IMX392, 1920×1200, global shutter, ~160 fps, C-mount | Widely stocked, GenICam/USB3 Vision, enough MP for cm-class fiducials at short WD, compact 29 mm cube |
| **Lens** | **Basler `C125-0618-5M-P`** (6 mm, order # `2000034831`) or **`C125-0818-5M-P`** (8 mm) | Matched C-mount for ≤1/2.5″; locking iris/focus; start 6 mm for wider FOV, 8 mm if markers look too small |
| **Compute** | **NVIDIA Jetson Orin Nano Super Developer Kit** (8 GB) | Cheap CUDA/CPU for OpenCV/AprilTag/ArUco @ ≥30 Hz; USB3 on carrier; JetPack Linux; fine for lab and early vehicle |

**Approx price band (ex accessories/cables)**

| Item | Band |
| --- | --- |
| Camera | ~**$360–470** / ~**€330–430** |
| Lens | ~**$180–250** / ~**€140–250** |
| Jetson Orin Nano Super Dev Kit | ~**$250–400** (MSRP recently moved in that range; check NVIDIA / SparkFun / Seeed) |
| **Rough kit total** | ~**$800–1,200** / ~**€700–1,100** |

**Buy also:** screw-lock USB3 Micro-B cable, rigid nadir bracket, optional LED ring. Dev kit is **not** vibration/IP-rated — expect a sealed industrial carrier later if this stack graduates to production.

**GigE alt (same family):** `a2A1920-160gmBAS` if cable length / multi-cam PoE matters later.

---

## Option B — Balanced field prototype (Hikrobot + NUC)

**Best for:** longer test drives, ROS 2 / x86 toolchain preference, still reasonable BOM.

| Piece | SKU / product | Why |
| --- | --- | --- |
| **Camera** | **Hikrobot `MV-CS050-60UM`** (or `MV-CS050-60UM V5`) — USB3, mono, global shutter, 2448×2048 (~5 MP), 2/3″, ~60 fps, C-mount | More pixels → better mm/px at same height; GS; USB3 Vision; often cheaper than EU premium brands for similar class |
| **Lens** | **C-mount 8–12 mm** rated for **2/3″** (e.g. Basler C23 / Computar / Kowa 2/3″ series — do **not** use a 1/2.5″-only lens on this sensor) | Larger sensor needs a larger image circle; pick FL so one marker still fills ≥50 px |
| **Compute** | **ASUS / Intel NUC 13 Pro Kit `NUC13ANHi5`** (Core i5-1340P barebone) + 16–32 GB DDR4 + 256–512 GB NVMe | Familiar Ubuntu/Windows, strong single-thread for detect+PnP, multiple USB 3.2 ports, small footprint; easy to swap if Jetson driver friction appears |

**Approx price band**

| Item | Band |
| --- | --- |
| Camera | ~**$500–850** / ~**€500–850** (US list often ~$529; EU quotes higher) |
| 2/3″ fixed lens | ~**$150–400** / ~**€150–350** |
| NUC13ANHi5 barebone | ~**$480–640** / ~**€460–640** |
| RAM + SSD (configured) | + ~**$100–250** |
| **Rough kit total** | ~**$1,200–2,000** / ~**€1,100–1,900** |

**FLIR alternate camera (similar band):** Teledyne FLIR **Blackfly S `BFS-U3-16S2M-CS`** — USB3, mono, IMX273 GS, 1440×1080, CS-mount (~**$370–560** / ~**€380**). Use a 5 mm CS→C spacer if buying C-mount glass. Slightly fewer pixels than Hikrobot; excellent Spinnaker/GenICam support.

**Caveat:** NUC is office/industrial-ambient, not shock-certified. For rough floors, put it in a damped enclosure or move to Option C IPC.

---

## Option C — Industrial / pilot (IDS + vision IPC) ← **industrial pick**

**Best for:** docking / 1 cm acceptance, glare/dirt, sustained onboard duty.

| Piece | SKU / product | Why |
| --- | --- | --- |
| **Camera** | **IDS uEye+ `U3-3060CP-M-GL` Rev.2** (item `AB00604`) — USB3, mono, Sony **IMX174**, 1936×1216, global shutter, C-mount, ~164 fps | Large 5.86 µm pixels → strong SNR / low noise under short exposure (anti-blur); proven industrial USB3 Vision; screw-lock Micro-B + Hirose I/O for strobe |
| **Lens** | **C-mount ~6–12 mm for ≥1/1.2″** (IDS / Edmund / Schneider / Basler C11 class) + optional polarizer | IMX174 is 1/1.2″ — match image circle; stop down for DOF if height wobbles |
| **Compute** | **Advantech `AIIS-3411U`** compact vision IPC (10th-gen Core, **4× dedicated USB3** camera channels) **or** AGV-oriented fanless mini-PC with 4–6× USB3 + 12–24 V DC (e.g. Partaker I37 / similar industrial NUC-class) | Dedicated USB3 bandwidth, DC input, serial/GPIO for lights & vehicle I/O; better sustained duty than a Dev Kit |

**Approx price band**

| Item | Band |
| --- | --- |
| IDS U3-3060CP-M-GL | ~**$1,000–1,150** / ~**€910–1,100** |
| 1/1.2″-capable lens + filter | ~**$200–500** |
| Advantech AIIS-3411U (list) | ~**$2,000+** (config-dependent) |
| Fanless industrial mini-PC (alt) | ~**$600–1,500** configured |
| **Rough kit total** | ~**$2,500–4,000+** / ~**€2,300–3,800+** |

**GigE alt:** same IDS family GigE models, or Basler ace 2 GigE, if cable length >~3–5 m USB3 or chassis EMI is an issue.

---

## Quick comparison

| | A Lab | B Balanced | C Industrial |
| --- | --- | --- | --- |
| Camera | Basler `a2A1920-160umBAS` | Hikrobot `MV-CS050-60UM` (or FLIR `BFS-U3-16S2M-CS`) | IDS `U3-3060CP-M-GL` |
| Shutter / IF | GS / USB3 | GS / USB3 | GS / USB3 |
| Compute | Jetson Orin Nano Super Dev Kit | NUC 13 Pro `NUC13ANHi5` | Advantech AIIS-3411U or fanless IPC |
| Kit band | ~$0.8–1.2k | ~$1.2–2k | ~$2.5–4k+ |
| Role | Budget / lab | Field prototype | Pilot / industrial |

---

## Shared BOM add-ons (any option)

- Screw-locking USB3 cable (or Cat6 + PoE injector if GigE)
- Rigid camera bracket (no soft rubber that drifts extrinsics)
- Diffuse LED ring or bar + optional circular polarizer pair for wet/glare floors
- Calibration target (Charuco / checkerboard) sized for working height
- Durable matte floor markers (separate from electronics BOM)

---

## Suggested buy order

1. Order **Option A** for static cm-repeatability on the bench (design milestone 1).  
2. If x86/ROS is preferred on-vehicle, move camera knowledge to **Option B**.  
3. Freeze **Option C** (or C camera + hardened compute) before docking / pilot-lane acceptance.

---

*Prices and stock checked against public distributor pages; re-quote before PO. No software deliverable in this doc.*
