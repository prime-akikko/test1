# AGV Indoor Localization via QR Floor Markers

Design-only. Target: ~1 cm planar accuracy. No UWB, no laser, no phone/web/desktop app.

---

## 1. Goal & constraints

**Goal:** Estimate AGV pose \((x, y, \theta)\) on a flat indoor floor using downward-looking camera + QR (or QR-like) markers fixed to the floor.

**Hard constraints**

| Constraint | Choice |
| --- | --- |
| Platform | AGV / AMR chassis (onboard compute + camera) |
| Sensing | Optical floor markers only (QR / ArUco / AprilTag-style fiducials) |
| Accuracy | ~1 cm in \(x,y\) under nominal conditions; heading useful for path following (~0.5–1°) |
| Explicitly out | UWB, LiDAR/laser SLAM, phone/web/desktop UI, warehouse WMS integration |

**Assumptions:** Indoor, mostly flat floor; controlled lighting preferable; markers surveyed into a known map; vehicle height above floor is roughly constant.

---

## 2. System architecture

```
Floor markers (ID + known pose in map)
        ↑ view
Downward camera (+ optional ring light)
        ↓ frames
Onboard vision pipeline (detect → decode → PnP/homography)
        ↓ pose sample + quality
Pose filter (fuse with wheel odometry / IMU between markers)
        ↓
Pose output interface → motion controller / Nav stack
```

**Blocks**

1. **Marker field** — durable printed codes on floor, each with unique ID and surveyed \((x,y,\theta_{\text{marker}})\) in a site map.
2. **Downward camera** — rigid mount under or on chassis, optical axis ≈ nadir; FOV sized so ≥1 marker is usually visible at cruise spacing.
3. **Onboard compute** — SBC/IPC running detection + pose + filter at ≥30 Hz preferred for control.
4. **Pose output** — time-stamped message: \(x, y, \theta\), covariance or quality flag, marker ID(s) used, latency. Suggested interface shapes (pick one when building): ROS 2 `nav_msgs/Odometry` / `geometry_msgs/PoseWithCovarianceStamped`, or a plain protobuf/UDP struct for a custom controller.

No cloud dependency for the localization loop.

---

## 3. Marker layout

### Size

- Physical module size drives pixel coverage at camera height \(h\).
- Rule of thumb: marker side \(s\) such that the code spans **≥40–60 px** across the quiet zone at operating height (more for harsh dirt/glare).
- Example at \(h \approx 150\)–250 mm with a moderate FOV lens: **80–150 mm** outer square is a common industrial starting point; validate with the chosen camera.

### Spacing

- Dense enough that the AGV rarely travels >1–2 m without a sighting if odometry drift is the limiter.
- Typical grid: **0.5–1.0 m** centers on main lanes; denser at docking / precision zones (0.25–0.5 m).
- Overlap FOV so at least one full marker is in view most of the time at max speed.

### Map / survey

- Build a **marker map**: `id → (x, y, yaw, size, plane_z)`.
- Survey with total station / calibrated tape grid / laser distance from surveyed monuments — not “eyeball place then hope.”
- Store map as static file on the AGV (YAML/JSON/SQLite). Version the map; reject unknown IDs.

### Coordinate frame

- **World / map:** right-handed, \(z\) up, \(xy\) floor; origin at a surveyed site point.
- **Marker:** origin at marker center; \(x\) along printed “up” axis for yaw reference.
- **Camera / robot:** extrinsic \(T_{\text{robot}}^{\text{cam}}\) from CAD + calibration; pose published in map frame as robot base link.

Consistency of frames matters more than which convention; document one and stick to it.

---

## 4. Pose estimation pipeline

1. **Capture** — global-shutter preferred; fixed exposure or auto with limits; optional IR + IR markers if glare is chronic.
2. **Detect** — find fiducial contours / candidate quads (OpenCV ArUco/AprilTag, or QR detectors). Prefer industrial fiducials (ArUco/AprilTag) over plain QR if pose from corners is primary; QR still fine if ID payload + four corners are reliable.
3. **Decode ID** — map ID → surveyed pose + physical size.
4. **Solve geometry**
   - **Homography** (planar floor assumption): image ↔ marker plane → camera pose relative to marker; compose with marker pose → map pose.
   - **PnP** (solvePnP / IPPE): 3D corner points of marker (known size, \(z=0\) plane) ↔ 2D image points → \(T_{\text{cam}}^{\text{marker}}\), then \(T_{\text{map}}^{\text{robot}}\).
5. **Multi-marker** — if ≥2 markers visible, fuse poses (weighted by reprojection error / distance from image center) for better yaw and outlier rejection.
6. **Output** — \((x, y, \theta)\) in map frame + quality (reprojection RMSE, #corners, age).

Pipeline runs onboard; controller consumes filtered pose, not raw detections alone.

---

## 5. How ~1 cm is achieved

Centimeter-class accuracy is a **budget**, not a single algorithm trick.

| Error source | Mitigation |
| --- | --- |
| Intrinsics / distortion | Checkerboard or Charuco calibration; re-calibrate after lens/focus change |
| Extrinsics (cam ↔ robot) | Rigid mount; calibrate with known floor motions or fixture |
| Print scale / stretch | Measure actual marker size after install; store per-ID size if needed |
| Height / focus | Fixed focus at working \(h\); stop down aperture for DOF if height varies slightly |
| Lens / FOV | Too wide → few px/mm and distortion; too narrow → misses markers. Aim for enough mm/px at floor |
| Corner localization | Subpixel corners; avoid motion blur (GS shutter, short exposure, light) |
| Between markers | Wheel odometry + IMU (EKF/UKF); QR resets absolute pose; drift bound by spacing × odom quality |
| Floor non-flat / mount flex | Limit; model small pitch/roll if needed; keep mount stiff |

**Order-of-magnitude:** if a marker spans ~100 mm and corners are localized to ~0.2–0.5 px with ~50–100 px across the marker, geometric error can sit in the few-mm range before survey and calibration residuals. Survey error and print quality often dominate — budget them explicitly (e.g. survey ≤3–5 mm, print ≤1 mm).

**Fusion:** treat each good marker sighting as an absolute measurement with covariance; propagate with odometry between sightings. That keeps path-following smooth and holds ~1 cm near markers / docking, with larger uncertainty mid-gap (still usually fine for aisle travel).

---

## 6. Failure modes & mitigations

| Failure | Symptom | Mitigation |
| --- | --- | --- |
| Dirt / scuff | Missed detect, bad corners | Durable laminate; scheduled clean; reject high reprojection error |
| Glare / wet floor | Hotspots, false quads | Matte laminate; polarized filter + polarized light; IR illumination |
| Missing / damaged marker | Gap in absolute fixes | Odom/IMU coast; max-coast distance → slow/stop; redundant neighbors |
| Occlusion (pallet, cable) | Temporary loss | Same as missing; don’t trust stale absolute pose |
| Latency | Control lag / oscillation | Pipeline budget (e.g. &lt;20–30 ms); timestamp frames; delay-compensate in filter |
| Wrong ID / map mismatch | Jump in pose | Gate large innovations; dual-marker agreement; map versioning |
| Motion blur at speed | Soft edges | GS shutter, light, exposure cap; slow in precision zones |
| Ambiguous / flipped pose | Wrong yaw | IPPE disambiguation; IMU prior; multi-marker |

Safe behavior: if quality stays bad beyond threshold → degrade to odom-only with rising covariance → stop or limp to last known good zone.

---

## 7. Hardware shortlist (recommendations)

Class-level guidance below. **Concrete orderable SKUs, compute boards, and rough EUR/USD bands** are in the companion doc:

→ **[Shopping shortlist — camera + CPU](./agv-qr-hw-shortlist.md)**  
  - **Budget / lab:** Basler `a2A1920-160umBAS` + Jetson Orin Nano Super Dev Kit  
  - **Balanced:** Hikrobot `MV-CS050-60UM` (or FLIR Blackfly S) + NUC 13 Pro  
  - **Industrial:** IDS `U3-3060CP-M-GL` + Advantech AIIS / fanless IPC  

**Camera / lens**

- Global-shutter industrial USB3/GigE, ~1–5 MP (monochrome for fiducials).
- Fixed focal length matched to height/FOV; low distortion C/CS-mount (typically 6–12 mm at ~150–250 mm working height).
- Optional: IR-pass + IR LEDs for lighting independence; polarizer kit for glare.

**Compute**

- Mid SBC / industrial PC (x86 or Jetson-class) with USB3/GigE and enough CPU for detect@30 Hz; GPU optional.
- Real-time OS not mandatory if latency is measured and bounded.

**Print / floor materials**

- High-contrast matte vinyl or polycarbonate labels with abrasion-resistant laminate.
- Adhesive rated for forklift/AGV traffic; or recessed inlay for high-wear lanes.
- Test chemical cleaners and UV if near windows.

**Mounting**

- Rigid camera bracket; protect from debris; keep lens cleanable.
- Avoid soft rubber mounts that change extrinsics over time.

---

## 8. Out of scope (this document / near term)

- Software implementation, ROS packages, or firmware
- UWB, LiDAR, visual SLAM as primary localization
- Phone / web / desktop operator apps
- Full fleet manager, traffic control, WMS
- Outdoor / non-flat / highly dynamic lighting without further study
- Safety certification (ISO 3691-4 etc.) — separate track

---

## 9. Suggested next milestones (when building)

1. **Lab bench** — one camera, one marker, calibration; measure static \(x,y\) repeatability vs ground truth tape/fixture.
2. **Extrinsic + robot frame** — mount on AGV; verify pose vs surveyed points while stationary.
3. **Marker map prototype** — small grid (e.g. 3×3); survey; drive slowly; log errors.
4. **Odometry fusion** — EKF; characterize drift vs spacing and speed.
5. **Robustness pass** — dirt, glare, missing marker, latency under load; define fail-safe.
6. **Docking / 1 cm zone** — denser markers; acceptance test protocol (RMSE, max error, % availability).
7. **Pilot lane** — production-like floor materials and lighting; freeze HW BOM.

---

## Decision summary

| Topic | Decision |
| --- | --- |
| Method | Downward camera + floor fiducials + odom/IMU fusion |
| Accuracy target | ~1 cm near markers / docking with calibrated system |
| Not used | UWB, laser, phone/web apps |
| Deliverable now | This design + [HW shopping shortlist](./agv-qr-hw-shortlist.md) — no code |

*End of design.*
