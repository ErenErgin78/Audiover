# Audiover — Fedora Real-Time Voice Changer & Soundboard Engine

**Audiover**, modern Linux sistemleri (özellikle **Fedora**, **PipeWire / WirePlumber** ve **Wayland**) için geliştirilmiş, profesyonel kalitede, ultra düşük gecikmeli (<10ms) gerçek zamanlı ses dönüştürücü (**Voice Changer**) ve çok kanallı ses tahtası (**Soundboard**) uygulamasıdır.

Mikrofon sesinizi anında tanınmaz hale getirebilir, stüdyo kalitesinde ses efektleri (DSP) uygulayabilir ve oyun oynarken veya Discord / OBS / Telegram üzerinden sohbet ederken ses ve video kliplerini tek tuşla karşı tarafa iletebilirsiniz.

---

## 🌟 Öne Çıkan Özellikler

### 1. 🎙 Gerçek Zamanlı DSP Ses Dönüştürücü (Voice Changer)
* **Granular Pitch Shifting:** Çift gecikmeli dairesel tampon (dual-delay circular buffer) ve Hann pencereli granül sentezi ile anlık ses kalınlaştırma (-12 semiton) veya inceltme (+12 semiton).
* **Cyborg / Robot:** Ayarlanabilir taşıyıcı frekansı (50 Hz - 600 Hz) ve miks oranı ile ring modülasyonu.
* **Walkie-Talkie & Telsiz:** 2. derece Butterworth bandpass filtreleme (300 Hz – 3400 Hz) ve harmonik distorsiyon saturasyonu.
* **Distortion & Drive:** Hiperbolik tanjant (`tanh`) tabanlı yumuşak analog doygunluk ve zenginleştirme.
* **Schroeder Reverb:** 4 paralel tarak filtre (comb filter) ve 2 seri tüm-geçiren (all-pass) difüzör ile katedral yankı etkisi.
* **Spatial Chorus:** LFO modülasyonlu gecikme çizgisi ile uzamsal koro efekti.
* **Akıllı Noise Gate:** Konuşma başlangıcında hızlı açılan (attack), bitişinde doğal sönen (release) yumuşak geçişli (soft-knee) gürültü kapısı.
* **Ses Ön Ayarları (Presets):** Ana ekranda büyük ve merkezi kartlar şeklinde **Clean** ve **Deep Voice** hızlı geçiş seçenekleri.
* **Özel DSP & Ön Ayar Ayarları (⚙ Settings):** Sağ üstteki ayarlar butonu ile açılan özel sayfadan yeni preset ekleme, silme ve tüm master DSP kontrollerini (Pitch, Ring Modülasyonu, Filtre, Distorsiyon, Reverb, Chorus, Noise Gate) gerçek zamanlı olarak yapılandırma.
* **Bypass Modu:** Tek tuşla tüm efektleri atlayıp ham mikrofona dönme.

### 2. 🎵 Çok Kanallı Ses Tahtası (Soundboard)
* **Geniş Format Desteği:** `.mp3`, `.wav`, `.ogg`, `.flac`, `.m4a`, `.mp4` (video dosyalarının yalnızca ses akışını anında ayrıştırır).
* **Doğrudan RAM'den Oynatma:** Tüm sesler belleğe 48 kHz 32-bit float stereo formatında yüklenir; disk gecikmesi olmadan sıfır tepki süresiyle tetiklenir.
* **Kanal Başına Kontrol:** Her ses için bağımsız ses düzeyi (Volume), döngü (Loop), ilerleme çubuğu (Progress bar) ve durdurma kontrolü.
* **Panik Butonu (Stop All Sounds):** Çalan tüm sesleri tek tuşla (`F11`) anında susturma.

### 3. 🔀 PipeWire / PulseAudio Akıllı Sanal Ses Yönlendirme
* **Otomatik Sanal Cihaz Yönetimi:** Uygulama açılışında `Audiover_Sink` (Sanal Çıkış) ve `Audiover_Mic` (`module-remap-source`) oluşturur.
* **Uygulama İzolasyonu:** Discord, OBS Studio, Steam veya oyunlarda mikrofon olarak **"Audiover_Virtual_Microphone"** seçildiğinde, dönüştürülen sesiniz ve soundboard efektleri mikslenerek temiz şekilde karşıya aktarılır.
* **Geri Bildirim Koruması:** Fiziksel mikrofon ile sanal aygıtlar filtrelenir; döngü (loopback feedback) oluşması engellenir.
* **Otomatik Temizleme (Graceful Cleanup):** Uygulama kapandığında oluşturulan sanal modüller sistemden otomatik olarak kaldırılır.

### 4. 🎧 Canlı İzleme (Monitoring / Hear Myself)
* **Hear Myself (`F8`):** Kendi dönüştürülmüş sesinizi gecikmesiz olarak kulaklığınızdan duyabilirsiniz.
* **Hear Soundboard:** Tetiklediğiniz ses efektlerinin kulaklığınıza da verilmesini açıp kapatabilirsiniz.
* Bağımsız monitör ses kazancı (Monitor Gain) ayarı.

### 5. ⌨ Global Kısayol Tuşları (Wayland / X11)
* Oyun oynarken veya tam ekran pencerelerdeyken uygulama arka planda olsa bile çalışan `/dev/input` donanım seviyesi olay dinleyicisi.
* Mikrofonu susturma (`F9`), Efektleri kapatma (`F10`), Tüm sesleri durdurma (`F11`) ve özel ses tuşları.

---

## 🛠 Teknik Mimari ve Çalışma Prensibi

Audiover, yüksek performanslı Python bilimsel kütüphaneleri (`numpy`, `scipy`), `sounddevice` (PortAudio/PipeWire) ve `PyQt6` üzerine inşa edilmiştir.

```
                      ┌───────────────────────────────┐
                      │    Fiziksel Mikrofon (I/O)    │
                      └──────────────┬────────────────┘
                                     │ (48kHz Float32 Block)
                                     ▼
                      ┌───────────────────────────────┐
                      │    DSP Ses İşleme Zinciri     │
                      │  - Noise Gate (Soft Knee)     │
                      │  - Granular Pitch Shifter     │
                      │  - Ring Modulator / Robot     │
                      │  - Butterworth Bandpass       │
                      │  - Saturation / Distortion    │
                      │  - Schroeder Reverb & Chorus  │
                      └──────────────┬────────────────┘
                                     │
 ┌─────────────────────────┐         │ (İşlenmiş Ses)
 │   Soundboard Engine     │         │
 │ (RAM 48kHz Stereo Mix)  ├─────────┼──────────────────────────────┐
 └───────────┬─────────────┘         │                              │
             │ (SFX Audio)           ▼                              ▼
             │                ┌──────────────┐              ┌──────────────┐
             └───────────────►│ Master Mixer │              │ Monitor Mix  │
                              │  + Limiter   │              │  + Gain Adj. │
                              └──────┬───────┘              └──────┬───────┘
                                     │                             │
                                     ▼                             ▼
                      ┌───────────────────────────────┐     ┌──────────────┐
                      │    Audiover_Sink (Null Sink)  │     │   Kulaklık   │
                      └──────────────┬────────────────┘     │   (Monitor)  │
                                     │                      └──────────────┘
                                     ▼
                      ┌───────────────────────────────┐
                      │ Audiover_Virtual_Microphone   │
                      │    (module-remap-source)      │
                      └──────────────┬────────────────┘
                                     │
                        ┌────────────┴────────────┐
                        ▼                         ▼
                 [Discord / OBS]          [Oyunlar / Steam]
```

### Teknik Parametreler
* **Örnekleme Hızı (Sample Rate):** 48,000 Hz (Stüdyo standardı).
* **Blok Boyutu (Block Size):** 256 frames (~5.33 ms donanım tampon gecikmesi).
* **Dahili Format:** Tek ve çift kanallı 32-bit kayan nokta (`float32`, [-1.0, 1.0]).
* **Kırpılma Koruması (Clipping Prevention):** Master çıkışta hiperbolik tanjant yumuşak sınırlayıcı (soft limiter).
* **Girdi İşleme (Input Event):** Linux `/dev/input/event*` standart `struct input_event` (24-byte `llHHI` formatı) ile Wayland compositor kısıtlamalarına takılmayan global kısayol takibi.

---

## 📁 Proje Dizin Yapısı

```text
Audiover/
├── assets/
│   └── sounds/                # Örnek ses efektleri (.wav, .mp3)
├── config/
│   └── settings.json          # Kayıtlı ayarlar, ses kütüphanesi ve kısayollar
├── src/
│   ├── main.py                # Ana uygulama başlatıcı ve yaşam döngüsü
│   ├── audio/
│   │   ├── dsp.py             # Düşük gecikmeli DSP efekt zinciri ve algoritmaları
│   │   ├── router.py          # PipeWire/PulseAudio sanal sink & source yöneticisi
│   │   └── stream.py          # Çok kanallı gerçek zamanlı I/O ve mikser motoru
│   ├── soundboard/
│   │   ├── player.py          # RAM içi çözücü (decoding), oynatıcı ve mikser
│   │   └── manager.py         # Ses kütüphanesi ve JSON kalıcılık yönetimi
│   ├── input/
│   │   └── hotkeys.py         # Wayland (evdev) / Linux ham girdi kısayol yöneticisi
│   └── ui/
│       ├── main_window.py     # PyQt6 ana pencere ve canlı VU metreler
│       ├── voice_panel.py     # DSP efekt kartları, slider'lar ve ön ayarlar
│       ├── soundboard_panel.py# Ses ızgarası, ilerleme çubukları ve medya yükleyici
│       ├── audio_settings_panel.py # Cihaz seçimi, gecikme ve kazanç ayarları
│       ├── hotkeys_panel.py   # Global kısayol tuşu atama arayüzü
│       └── styles.py          # Modern Dark / Neon siberpunk QSS teması
├── requirements.txt           # Python bağımlılıkları
├── setup_env.sh               # Otomatik sanal ortam ve bağımlılık kurulum scripti
├── run.sh                     # Uygulama başlatma scripti
└── README.md
```

---

## 🚀 Kurulum ve Başlangıç

### Sistem Gereksinimleri
* **İşletim Sistemi:** Linux (Fedora 38/39/40+, Arch, Ubuntu 22.04+ vb.)
* **Ses Sunucusu:** PipeWire (`pipewire-pulse`) veya PulseAudio
* **Gerekli Sistem Araçları:** `pulseaudio-utils` (`pactl`), `ffmpeg`
* **Python:** Python 3.10 veya üzeri

### 1. Kurulum
Proje kök dizininde kurulum scriptini çalıştırın:
```bash
chmod +x setup_env.sh run.sh
./setup_env.sh
```
*Bu script sanal ortamı (`.venv`) oluşturur ve `requirements.txt` içerisindeki tüm kütüphaneleri (PyQt6, sounddevice, scipy, numpy, soundfile, miniaudio vb.) yükler.*

### 2. Wayland Global Kısayol İzni (Opsiyonel ama Önerilen)
Wayland üzerinde oyunlardayken global kısayolların `/dev/input` üzerinden dinlenebilmesi için kullanıcınızın `input` grubunda olması gerekir:
```bash
sudo usermod -aG input $USER
```
*(Komutu çalıştırdıktan sonra oturumu bir kez kapatıp açmanız yeterlidir).*

### 3. Uygulamayı Başlatma
```bash
./run.sh
```
*(Veya doğrudan: `.venv/bin/python src/main.py`)*

---

## 🎙 Discord, OBS ve Oyun Yapılandırması

1. **Audiover** uygulamasını açın ve **ENGINE ACTIVE** durumunda olduğunu teyit edin.
2. **Discord** (veya oyun içi ses ayarları):
   * **Giriş Aygıtı (Input Device):** `Audiover_Virtual_Microphone` (veya `Audiover_Mic`)
   * **Çıkış Aygıtı (Output Device):** Fiziksel Kulaklığınız
3. **OBS Studio:**
   * Ses Giriş Yakalayıcısı (Mic/Aux) olarak `Audiover_Virtual_Microphone` seçin.

---

## ⌨ Varsayılan Global Kısayollar

| Tuş | Eylem |
|---|---|
| `F8` | **Hear Myself (Loopback):** Kendi sesini dinlemeyi aç / kapat |
| `F9` | **Mute:** Mikrofonu anında sustur / aç |
| `F10` | **Bypass DSP:** Efektleri devre dışı bırak / etkinleştir |
| `F11` | **Panic Button:** Çalan tüm soundboard seslerini anında sustur |
| `1` | Airhorn SFX Çal |
| `2` | Level Up Chime Çal |
| `3` | Cyber Alert SFX Çal |

*Tüm kısayollar Audiover arayüzündeki **Hotkeys** sekmesinden özelleştirilebilir.*

---

## 📄 Lisans
Bu proje açık kaynaklıdır ve MIT Lisansı altında sunulmaktadır.
