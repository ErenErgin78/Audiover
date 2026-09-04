<div align="center">

<img src="assets/icons/audiover.png" alt="Audiover Logo" width="128" />

# AUDIOVER

**Linux İçin Gerçek Zamanlı Ses Değiştirici Ve Soundboard Uygulaması**

<br/>

[![][github-release-shield]][github-release-link]
[![][github-release-date-shield]][github-release-link]
[![][github-downloads-shield]][github-downloads-link]
[![][github-downloads-latest-shield]][github-downloads-link]

</div>

---

## Kurulum

### Hazır Paketler ile Kurulum

[Releases][github-release-link] sayfasından dağıtımınıza uygun sürümü indirin:

- **AppImage:**
  ```bash
  chmod +x Audiover_*.AppImage
  ./Audiover_*.AppImage
  ```

- **Debian / Ubuntu (.deb):**
  ```bash
  sudo dpkg -i audiover_*.deb
  ```

- **Fedora / RHEL (.rpm):**
  ```bash
  sudo dnf install ./audiover-*.rpm
  ```

### Hızlı Başlangıç (Kaynak Koddan)

Projeyi doğrudan klonlayıp çalıştırabilirsiniz:

```bash
git clone https://github.com/ErenErgin78/Audiover.git
cd Audiover
make setup-deps
make dev
```

### Ses Yapılandırması

1. Audiover uygulamasını başlatın.
2. Hedef uygulamada (Discord, OBS Studio vb.) mikrofon / giriş aygıtı olarak **Audiover_Virtual_Microphone** (veya `Audiover_Mic`) seçin.
3. Çıkış aygıtı olarak kulaklığınızı seçin.

---

## Özellikler

- **Rust ve Tauri Altyapısı:** Sistem kaynaklarını minimum düzeyde tüketen hafif ve yüksek performanslı mimari.
- **Canlı Ses Değiştirici:** Ses perdesi, robot sesi, telsiz filtresi, yankı ve doygunluk gibi kişiselleştirilebilir ayarlar.
- **Dahili Soundboard:** MP3, WAV, OGG, FLAC ve M4A formatlarındaki ses efektleri eklenip oynatılabilir.
- **Canlı Dinleme:** Mikrofonunuzdan geçen işlenmiş sesi ve çalan efektleri kulaklığınızdan anlık olarak duyma özelliği.
- **Sanal Mikrofon:** Discord, OBS, oyunlar ve tüm iletişim yazılımlarıyla tam uyumluluk.
- **Global Kısayollar:** Uygulama simge durumundayken veya oyundayken dahi ses efektlerini ve ses profilini yönetin.

---

## Lisans

Bu proje [MIT](LICENSE) lisansı altında sunulmaktadır.

<!-- Bağlantı ve Rozet Tanımları -->
[github-release-shield]: https://img.shields.io/github/v/release/ErenErgin78/Audiover?style=flat-square
[github-release-date-shield]: https://img.shields.io/github/release-date/ErenErgin78/Audiover?style=flat-square
[github-downloads-shield]: https://img.shields.io/github/downloads/ErenErgin78/Audiover/total?style=flat-square
[github-downloads-latest-shield]: https://img.shields.io/github/downloads/ErenErgin78/Audiover/latest/total?style=flat-square
[github-release-link]: https://github.com/ErenErgin78/Audiover/releases
[github-downloads-link]: https://github.com/ErenErgin78/Audiover/releases

