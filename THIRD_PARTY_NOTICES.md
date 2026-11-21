# Third-Party Notices

**BreezeMusic Desktop** is licensed under the GNU General Public License v3.0 (GPL-3.0).
This project incorporates or references open-source research and libraries from the community. We gratefully acknowledge the following projects and their authors:

---

## 1. Protocol Research Reference

* **MakcRe/KuGouMusicApi**
  * Repository: https://github.com/MakcRe/KuGouMusicApi
  * License: MIT License
  * Note: This application contains an independent pure-Rust client-side protocol implementation informed by the protocol documentation and endpoint studies in `MakcRe/KuGouMusicApi`. No upstream JavaScript/Node.js source files are bundled in this repository.

---

## 2. Core Open-Source Dependencies

### Frontend (JavaScript / TypeScript)
* **Vue 3** — MIT License (https://vuejs.org/)
* **Pinia** — MIT License (https://pinia.vuejs.org/)
* **Vite** — MIT License (https://vitejs.dev/)
* **TypeScript** — Apache-2.0 License (https://www.typescriptlang.org/)
* **@tauri-apps/api & @tauri-apps/cli** — Apache-2.0 OR MIT License (https://tauri.app/)
* **lucide-vue-next** — ISC License (https://lucide.dev/)

### Native Backend (Rust / Tauri)
* **Tauri 2.0 (`tauri`, `tauri-build`)** — Apache-2.0 OR MIT License
* **Tokio** — MIT License
* **Serde / serde_json** — Apache-2.0 OR MIT License
* **Reqwest** — Apache-2.0 OR MIT License
* **lofty** — Apache-2.0 OR MIT License (audio metadata & tag engine)
* **RustCrypto crates (`aes`, `cbc`, `md-5`, `sha2`, `rsa`, `zeroize`)** — Apache-2.0 OR MIT License
* **webview2-com / windows-core** — Apache-2.0 OR MIT License

Full dependency trees and exact resolved versions are recorded in `package-lock.json` and `src-tauri/Cargo.lock`.
