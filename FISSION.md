# Fission's Vello fork

`fission` is the integration branch for Fission's renderer changes. Start new
fixes from this branch and merge them back here. Application repositories should
not select an old topic branch to recover one fix while losing another.

## Reconciled branches

This branch starts from the fork's `main` and brings together these histories:

| Branch | Integrated behavior |
| --- | --- |
| `worka/dynamic-gpu-buffers` | Historical mobile dispatch and buffer work; retained where applicable to the active renderer. |
| `fission/sparse-strips` | Sparse CPU/GPU rendering, colour matrices and filter chains, backdrop filters, opposite-parity backdrop children, and fallible image uploads. |
| `codex/swiftshader-buffer-upload-20261111` | Unmapped transient upload buffers and queue-based layer configuration updates. |
| `codex/zoffice-release-vello-staging` | Bounded strip uploads, distinct scheduled strip ranges, batched queue submissions, and texture row-pitch handling. |
| `fix-webgpu-fallback-diagnostics` | History reconciled; its remaining Classic-only changes are deliberately superseded, not restored. |

The last branch's unique changes are the Classic profiled-PTCL allocation fix
and Classic release metadata. Commit `b5cd67cc` deliberately retired the Classic
workload-profile allocator when Fission adopted the sparse renderers. Classic
now lives under `research/`; Fission does not depend on it. Reintroducing that
allocator would revive an unused architecture, not restore an active sparse
renderer feature. The reconciliation merge records that decision explicitly.

## Validation

Run the active renderer unit suites from the Fission checkout, using its normal
Cargo target and cache directories:

```sh
cargo test --locked --manifest-path third_party/vello/Cargo.toml \
  -p fission-vello-common -p fission-vello-cpu -p fission-vello-gpu \
  --lib -- --test-threads=1
```

The reconciliation passed 386 tests (168 common, 85 CPU, 133 GPU); one upstream
SIMD accuracy test is explicitly ignored. These include filter, image-atlas,
backdrop parity, strip order, bounded upload ranges and texture layout checks.
Unit tests do not establish browser or graphics-driver compatibility. Also run
the fork's rendered CPU/GPU filter comparisons and Fission's renderer/input
integration suites when changing the relevant paths.

## Updating Fission

Fission's `.gitmodules` tracks `fission`. Its Cargo manifests use the single
checked-out fork through path dependencies, with registry versions retained for
crate publication. Do not replace one renderer dependency with a separate Git
source: doing so can link two copies of shared Vello types.

Git still records the tested submodule commit. A tracking branch is the source
of updates, not permission to change a build's source halfway through it:

```sh
git submodule update --init --recursive
git submodule update --remote third_party/vello
# Review and test the update, then commit the submodule change in Fission.
```

Keep the regression tests with each fix. Do not assume that sharing ancestry
alone proves the behavior still works, and do not roll the integration branch
back to an individual application's old topic branch.
