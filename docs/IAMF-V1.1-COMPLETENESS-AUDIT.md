# IAMF v1.1.0 – Vollständigkeits- und Korrektheitsaudit

**Stand:** 12. September 2026  
**Untersuchte Implementierung:** Arbeitsstand feature/aac-lc-framing  
**Normative Quelle:** [IAMF v1.1.0](iamf/v1.1.0.html), AOM Final Deliverable vom 24. Oktober 2024  
**Scope:** Rust-Modell, Parser und Standalone-IA-Sequence-Writer. Kein Decoder, Renderer oder ISO-BMFF-Packager.

## Ergebnis

iamf-rs ist für seinen bewusst begrenzten Auftrag – IAMF-v1.1-OBUs modellieren, validieren, parsen und als Standalone IA Sequence schreiben – weitgehend und nachvollziehbar implementiert. Die wichtigste Restlücke ist eine explizite Grenze zwischen absichtlich erlaubten Descriptor-Fragmenten und einer vollständigen, auslieferbaren IA Sequence. Decoder, Renderer, OAR und ISO-BMFF sind keine fehlenden Details dieser Crate, sondern bewusst getrennte Komponenten.

## Quellen und Aussagekraft

| Rolle | Quelle | Verwendung |
|---|---|---|
| Normativer Maßstab | docs/iamf/v1.1.0.html | Alle MUST/SHALL-Anforderungen dieses Berichts. |
| Implementierung | Arbeitsstand feature/aac-lc-framing | Tatsächlich geprüfter Rust-Code einschließlich der noch nicht committeten AAC-/Framing-/Profiländerungen. |
| Referenzorakel | [REFERENCES.md](../REFERENCES.md): iamf-tools@848c6ff… (v2.1.0), libiamf@f06e919… (v1.1.0) | Interoperabilität und Abgleich, nicht Ersatz für die Spezifikation. |

Die lokalen Klone docs/iamf-tools und docs/libiamf stehen derzeit auf späteren HEADs (d13b8dd… beziehungsweise e55e183…), nicht auf den Pins. Sie zeigen nützlichen Zukunfts-Drift, sind aber **kein** v1.1-Sollwert; besonders iamf-tools-HEAD enthält neuere Profile.

Der Eclipsa-Audio-Plugin-Quellbaum ist nun unter docs/eclipsa-audio-plugin am Commit c964609… vorhanden und wurde direkt untersucht. Seine eingebettete iamf-tools-Kopie weist Commit 7542365… aus. Das ist ebenfalls nicht der für iamf-rs gepinnte v1.1-Referenzstand. Eclipsa ist daher ein wertvoller Architektur- und Integrationsvergleich, aber keine normative Autorität.

| Status | Bedeutung |
|---|---|
| **Erfüllt** | Syntax und Bibliotheksverantwortung implementiert und abgesichert. |
| **Teilweise** | Wire-Modell vorhanden, aber High-Level-Autorierung oder Validierung begrenzt. |
| **Außerhalb des Scopes** | Gehört gemäß Architektur in Decoder, Renderer oder Packager. |
| **Offen** | Relevante Writer-Konformitätsprüfung oder ihr Beleg fehlt noch. |

## Priorisierte Befunde

| Priorität | Befund | Auswirkung | Empfehlung |
|---|---|---|---|
| **P1** | Descriptor-Fragmente bleiben für Streaming erlaubt, können aber mit `validate_delivery_conformance()` explizit von vollständigen Liefersequenzen getrennt werden. | Container-/Datei-Caller erhalten eine klare statische Grenze, ohne die Streaming-API zu brechen. | Vor dem Muxen immer Delivery- und Timeline-Validierung aufrufen. |
| **P1** | `validate_delivery_timeline()` prüft vor dem Schreiben jede bestehende Frame-/Parameter-Invariante sowie Start-/End-Trim-Positionen. Ein Renderer-spezifischer Beweis gemeinsamer Presentation-Zeitachsen bleibt bewusst außerhalb dieser Writer-Crate. | Ungültige Trim-Positionen werden vor einem Caller-Sink abgewiesen; Playback-Semantik bleibt Decoder/Rendererarbeit. | Für ISO-BMFF den Validator als Eingangsgate verwenden; spätere Renderer-Crate testet §7-Zeitachsen. |
| **P2** | Die Kernfälle der v1.1-Profilmatrix sind als Builder-Regressionen vorhanden und auf `iamf-tools@v2.1.0` provenance-festgelegt. | Simple/Base/Base Enhanced sowie Expanded, Reserved, Submix- und Codec-Config-Grenzen sind gegen Draft-Drift geschützt. | Bei jeder Profiländerung dieselben Tests plus gepinnten Docker-Referenzgate ausführen. |
| **P2** | High-Level-Authoring priorisiert sichere Standardfälle; skalierbare Mehrschicht-Layouts sowie Demixing-/Recon-Gain-Autorierung sind nicht gleich vollständig ergonomisch. | Kein Parser-/Wire-Defekt, aber komplexe Produzenten brauchen niedrigere APIs oder werden abgewiesen. | Nur bei Bedarf als eigenständige API-Erweiterung mit Profil-/Referenztests. |
| **P3** | Erweiterungs- und Redundanzpfade sind am Wire-Level unterstützt, aber keine komfortable Produktionsoberfläche. | Vorwärtskompatibel, aber nicht als „lossless editor“ positioniert. | Dokumentieren; erst bei Produktbedarf ausbauen. |

## Punkt-für-Punkt-Abgleich

### §2 – IA-Modell, Architektur und Timing

| Standardthema | Stand in iamf-rs | Bewertung |
|---|---|---|
| OBU Packetizer erzeugt IA Sequence aus Descriptor-, Audio- und Parameterdaten (§2.2). | EncoderBuilder friert Descriptoren/IDs ein; EncodingWriter schreibt Prolog und TemporalUnitInputs. | **Erfüllt** |
| OBU Parser gibt Descriptor-, Audio- und Parameterdaten aus. | Reader/OBU-Module bilden die OBU-Familien ab; Parameterblöcke verlangen Descriptor-Kontext. | **Erfüllt** |
| Implied timestamps, konsekutive Frames/Parameterblöcke, gleiche Dauer/Startzeit aller Mix Presentations (§2.4). | `validate_delivery_timeline()` führt dieselben nicht-schreibenden Frame-/Parameter-Prüfungen wie der Writer aus und erzwingt die finite Trim-Position. Keine globale Presentation-Zeitachsen-Ableitung. | **Teilweise** – Writer-/Delivery-Invarianten erfüllt; Renderer-Zeitachse außerhalb des Scopes |
| Codec Decoder, Element Reconstructor, Renderer, Mixer, Post-Processor (§2.2/§7). | Nicht implementiert und ausdrücklich aus README/HANDOFF ausgeschlossen. | **Außerhalb des Scopes** |

Die Trennung ist korrekt: Writer-Semantik endet beim Paketisieren. Das Anwenden von Gain-Animation, Rekonstruktion und binauralem/Lautsprecher-Rendering ist Decoder-/Rendererarbeit.

### §3.1–§3.3 – OBU-Grundformat und Header

| Anforderung | Stand | Bewertung |
|---|---|---|
| OBU-Typ, Extension-/Redundant-Copy-Flags, Größe und Payload | ObuHeader, Bitreader/-writer und Tests modellieren Header-Syntax und Grenzen. | **Erfüllt** |
| Bekannte, Reserved- und erweiterbare OBU-Daten | Typisierte bekannte Varianten sowie Raw-/Reserved-Erhaltung an Erweiterungsstellen. | **Erfüllt** für Parsing/Round-trip |
| Redundante Kopien erzeugen | Flag repräsentiert, kein automatischer Redundanzgenerator. | **Erfüllt** – Generator ist nicht gefordert |

### §3.4 – IA Sequence Header OBU

| Anforderung | Stand | Bewertung |
|---|---|---|
| Primary/Additional Profile wire-syntax | Typisierte Simple/Base/Base-Enhanced-Werte; Reserved bleibt klar getrennt. | **Erfüllt** |
| Deterministische Profilwahl | select_sequence_profile() schneidet Kandidaten anhand von Codec Config, Elements, Layout und Presentation ein. | **Teilweise** – Matrix absichern |
| Vollständige Sequence startet mit Header (§5.1). | Encoder::start() schreibt den gefrorenen Header-Prolog. | **Erfüllt** |

### §3.5 und §3.11 – Codec Config und Codec-spezifische Syntax

| Codec | Stand | Bewertung |
|---|---|---|
| LPCM | Typisierte Config, Frame-Plan und PCM-Frame-Writing. | **Erfüllt** als Writer |
| FLAC | Typisierte Config und vorcodierte Access Units. | **Erfüllt** als Writer |
| Opus | Typisierte Config und vorcodierte Access Units. | **Erfüllt** als Writer |
| AAC-LC | AudioSpecificConfig/Codec Config sowie IAMF-konformes Frame-Framing sind im untersuchten Branch vorhanden; Referenz-Conformance deckt den strikten Decoderpfad ab. | **Erfüllt** als v1.1-Wire-Format |
| Codec-Kompression/-decoding | Nicht enthalten. | **Außerhalb des Scopes** |

IAMF definiert hier Decoder-Setup und die Einbettung codierter Frames. Eine Bitstreambibliothek muss AAC, FLAC oder Opus nicht selbst komprimieren oder dekodieren; der Caller liefert Access Units, die Crate validiert und paketiert.

### §3.6 – Audio Element OBU

| Anforderung | Stand | Bewertung |
|---|---|---|
| Channel-based Elements, Substreams, Config-Referenzen und Layouts | Vollständiges Wire-Modell, Referenzprüfung und stabile Handle→ID-Zuordnung. | **Erfüllt** für Standardfälle |
| Scalable channel layout/mehrere Layer, Demixing, Recon Gain | Parser/Serializer und Parameterdaten kennen die Struktur; High-Level-Builder konzentriert sich auf sichere Ein-Layer-Pfade. | **Teilweise** |
| Scene-based Ambisonics | Mono und Projection im Datenmodell/Wire-Scope; Builder-Hilfe für Mono. | **Teilweise** als Authoring API |
| Reserved Element-/Ambisonics-Typen | Nicht als bekannte Bedeutung fehlinterpretiert; profilunverträglich behandelt. | **Erfüllt** defensiv |

### §3.7 – Mix Presentation OBU

| Anforderung | Stand | Bewertung |
|---|---|---|
| Submixes, Elements, Layouts, Rendering Config, Labels/Annotations, Loudness | Typisiertes Modell; Builder prüft Referenzen und eingefrorene Parameterzuordnung. | **Erfüllt** im Descriptor-Scope |
| Headphones Rendering Mode | Modelliert; Reserved-Werte führen zu keiner v1.1-Profilkandidatur. | **Erfüllt** |
| Submix-/Profilgrenzen | Mehr als eine Submix wird für v1.1 konservativ abgewiesen; der explizite Delivery-Validator verlangt mindestens eine Mix Presentation. Null Mix Presentations bleiben ausschließlich für die Fragment-API möglich. | **Erfüllt** für vollständige Delivery-Sequenzen |
| Loudness messen | Werte werden serialisiert, Messung findet nicht hier statt. | **Erfüllt** als Writer |

### §3.8 – Parameter Block OBU

| Anforderung | Stand | Bewertung |
|---|---|---|
| Parameter-ID/Definition kontextsensitiv auswerten | read_parameter_block() verlangt Registry/Context; ein Block kann nicht ohne Descriptor-Kontext fehlgedeutet werden. | **Erfüllt** |
| Mix Gain inklusive Step/Linear/Bezier | Typisierte Daten und Read/Write-Validierung. | **Erfüllt** |
| Demixing- und Reconstruction-Gain-Daten | Typisierte Daten, Kontextprüfung und Read/Write-Pfade. | **Erfüllt** auf Wire-Ebene |
| Animation anwenden | Kein DSP/Renderer. | **Außerhalb des Scopes** |
| High-Level-Erzeugung aller Parameterarten | Mix Gain ist der ergonomische Hauptpfad; weitere Typen liegen niedriger im Modell. | **Teilweise** |

### §3.9–§3.10 – Audio Frame und Temporal Delimiter

| Anforderung | Stand | Bewertung |
|---|---|---|
| Implizite/explizite Substream-IDs | Frame-Plan unterstützt beide Formen und prüft eingefrorene Descriptoren. | **Erfüllt** |
| Start-/End-Trimming | Trimming ist modelliert und beim Schreiben validiert; Sample-Entfernung ist Decoderarbeit. | **Erfüllt** als Writer |
| Temporal Units und Delimiter | Writer prüft und schreibt Blocks/Frames in definierter Unit-Reihenfolge. | **Erfüllt** |
| Sequenzweite Gleichheit von Präsentationszeitachsen | Der Writer prüft einzelne finite Units und Trim-Reihenfolge; ein globaler Presentation-/Renderer-Zeitachsenbeweis wird nicht modelliert. | **Teilweise** – bewusst Decoder-/Renderer-Scope |

### §4 – Profile

Die drei v1.1-Profile werden gezielt ohne spätere Draft-Profile ausgegeben. Der Filter berücksichtigt eindeutige Codec Configs, Element-/Kanallimits, Expanded Layouts, Elementtypen und Headphones-Modi. Primary und Additional Profile werden gleich gewählt – konservativ und für die unterstützte v1.1-Erzeugung passend.

| Profilaspekt | Stand | Bewertung |
|---|---|---|
| Nur v1.1-Profile emittieren | Simple, Base, Base Enhanced explizit; kein Draft-v2-Profil. | **Erfüllt** |
| Codec-Config- und Dimensionsgrenzen | Im Selektor geprüft. | **Erfüllt** |
| Mehr als eine Submix | Konservativ abgewiesen. Die Spezifikation/Referenz formuliert teils SHOULD ignore; ein Writer darf enger sein, muss dies aber dokumentieren. | **Erfüllt, konservativ** |
| Vollständige Referenzmatrix | Kernfälle sind als provenance-festgelegte Builder-Regressionen konserviert; eine erschöpfende differentielle Matrix für jeden komplexen Authoring-Pfad fehlt. | **Teilweise** (P2) |
| Fragment ohne Mix Presentation | Für Streaming weiterhin erlaubt; `validate_delivery_conformance()` lehnt es für eine Deliverable Sequence typisiert ab. | **Erfüllt** für den expliziten Delivery-Pfad |

### §5 – Standalone IA Sequence

| Anforderung | Stand | Bewertung |
|---|---|---|
| Header, Descriptor-Prolog, anschließend temporale Daten | Encoder hält diese Trennung ein. | **Erfüllt** |
| Referenzen und IDs konsistent | Opaque Handles werden deterministisch in Wire-IDs übersetzt und beim Build/Write validiert. | **Erfüllt** |
| Mehrere Konfigurationsabschnitte/Descriptor-Updates | OBU-Ebene flexibel; High-Level API ist bewusst freeze once, append temporal units. | **Teilweise** |
| Jede High-Level-Ausgabe ein vollständiger Lieferbitstream | `start()` bleibt absichtlich ein Fragment-/Streaming-Pfad. Der explizite Delivery-Pfad erzwingt vollständige Descriptoren und kann vor dem Sink die gesamte finite Timeline prüfen. | **Teilweise** – vollständige Lieferung ist opt-in, nicht implizit |

### §6 – ISO-BMFF IAMF Encapsulation

Nicht implementiert. Die Crate erzeugt bewusst Standalone IA Sequences, keine ISO-BMFF-Boxen, Sample Entries oder Containerzeitachsen.

**Bewertung: Außerhalb des Scopes.** Falls erforderlich: eigene iamf-isobmff-Crate oder klar abgegrenztes Paket.

### §7 – IAMF Processing

§7 verlangt Decoder-Setup, Audio-Decode, Element-Reconstruction, Parameteranimation, Lautsprecher-/Binaural-Rendering, Mixing und Post-Processing. iamf-rs trägt die nötigen OBU-/Descriptor-Daten, erzeugt daraus aber bewusst kein Audio.

**Bewertung: Außerhalb des Scopes.** Die sinnvolle Architektur bleibt:

~~~
iamf-rs                 Bitstreammodell, Parser, Standalone Writer
iamf-decode-rs          Codec-Decoding und Element-Reconstruction
iamf-render-rs          normative OAR-orientierte Rendering-/Mix-Pipeline
iamf-isobmff (später)   Containerisierung
~~~

Ein Renderer sollte auf dekodierten Element-Puffern arbeiten, nicht an der Writer-API hängen. Dadurch bleiben Bitstream-Konformität und DSP-/Realtime-Anforderungen unabhängig testbar.

## Direkter Eclipsa-Vergleich

Der nun vorliegende Quellbaum bestätigt die beabsichtigte Trennung zwischen
Authoring/Realtime-Rendering und der IAMF-Bitstreambibliothek. Die folgenden
Befunde sind direkte Codebeobachtungen, keine Hochrechnung aus einer README.

| Eclipsa-Baustein | Beobachtung | Konsequenz für iamf-rs |
|---|---|---|
| Export | IAMFFileWriter erzeugt Protobuf-UserMetadata und delegiert die tatsächliche Dateierzeugung an IamfEncoderFactory aus eingebettetem iamf-tools. | iamf-rs ersetzt genau diese Abhängigkeit als native Writer-Schicht; es sollte nicht die JUCE-/Repository-Logik übernehmen. |
| Codec-Auswahl | Der Exportpfad bietet LPCM, FLAC und Opus; AAC-LC ist dort nicht auswählbar. | Die AAC-LC-Erweiterung von iamf-rs ist zusätzlicher v1.1-Funktionsumfang, nicht eine Abweichung von Eclipsa. |
| Audio Elements | Eclipsa schreibt einen Codec-Config-Identifier und genau eine scalable-channel-layout-Schicht; Ambisonics ist dort ausdrücklich Mono. | Die High-Level-Grenzen von iamf-rs sind vergleichbar konservativ. Projection und Multi-Layer bleiben geplante, separate Authoring-Arbeit. |
| Mix Presentations | Eclipsa führt eigene Repository-IDs, Mix-Gain, Tags, Loudness und binaural-Flag; beim Export werden diese in iamf-tools-Metadaten überführt. | Das bestätigt die API-Grenze: iamf-rs nimmt fertige IAMF-Descriptoren an, besitzt aber keine DAW-Identität, UI- oder Persistenzschicht. |
| Zeitliche Kodierung | Eclipsa puffert Opus auf 960 Samples bei 48 kHz und delegiert Temporal Units an iamf-tools. | Die getrennte Frame-Plan-/Temporal-Unit-API in iamf-rs ist richtig; codec-spezifisches PCM-Puffern bleibt beim Adapter. |
| Renderer-Auswahl | RendererFactory trennt binaural, Channel-to-Channel/Bed-to-Bed, Passthrough und HOA-to-Bed. | Stützt die geplante eigenständige iamf-render-rs-Crate; diese Verantwortlichkeiten gehören nicht in den OBU-Writer. |
| Binaural | BinauralRdr verwendet OBR, akzeptiert konkrete Bed-/HOA-Layouts und verweigert Blöcke unter 32 Samples. | Das ist eine produkt-/Bibliotheksabhängige Realtime-Implementierung, nicht automatisch normatives OAR-Verhalten. Eine Rust-Implementierung muss §7/OAR separat spezifizieren und testen. |
| Quellenpositionierung | AmbisonicPanner panned explizit nur einen Mono-Eingang in HOA; die aktuelle Position wird blockweise an den OBR-Encoder übergeben. | Kein Ersatz für IAMF-Parameterblock-Animation oder eine normative Playback-Zeitachse. Gerade deshalb bleibt der P1-Timeline-Befund bestehen. |

Wichtige Fundstellen im Eclipsa-Tree:

- common/processors/file_output/iamf_export_utils/IAMFFileWriter.cpp – Codecwahl, Protobuf-Metadaten, Delegation an iamf-tools und Opus-Akkumulation.
- common/data_structures/src/AudioElement.cpp – Ein-Layer-Layout und Mono-Ambisonics im Export.
- common/data_structures/src/MixPresentation.h – Mix-Gain, Tags, Loudness und Rendering-Konfiguration als Authoring-Zustand.
- common/substream_rdr/rdr_factory/RendererFactory.cpp – die vier Rendererpfade.
- common/substream_rdr/bin_rdr/BinauralRdr.cpp und surround_panner/AmbisonicPanner.cpp – OBR-gebundenes binaurales Rendering und blockweises Mono-Panning.

Damit korrigiert der direkte Vergleich keine der priorisierten P1/P2-Feststellungen. Er schärft jedoch die Architekturentscheidung: Eclipsa kann als späterer Client oder als Referenz für Adapteranforderungen dienen, nicht als Modul, das in diese Crate hineinkopiert werden sollte.

### Annex A und informative Anhänge

Die IAMF-Generation-Pipeline ist informativ. Sie erklärt Preprocessing, Channel Groups, Codec Encoder und Packetizer, erweitert aber nicht die normative Writer-Syntax. iamf-rs übernimmt sinnvoll den Packetizer-Anteil. Codec-Encoding, Loudness-Messung und räumliche/künstlerische Aufbereitung gehören zum Caller.

## Referenz- und Testbewertung

Die Teststrategie ist für eine Beta belastbar:

- libiamf@v1.1.0 ist als gepinntes Decoderorakel vorgesehen; PCM-identischer Decode prüft mehr als Self-Roundtrip.
- iamf-tools@v2.1.0 dient als gepinnter strenger Parser-/Encoder-Referenzpunkt.
- Conformance-Fixures, Manifestprüfungen und Docker-Läufe trennen externe Toolchains von normalen Offline-Rust-Tests.
- Parser-, Boundary-, Fuzz-Regressions- und API-Tests ergänzen die Referenzläufe.

Für den untersuchten Branch wurde erfolgreich ausgeführt:

~~~
cargo test --locked -q
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
git diff --check
~~~

Zusätzlich wurde am 2026-09-12 der gepinnte `libiamf@f06e919…`-Build auf
macOS erfolgreich gebaut; sein unabhängiger LPCM-Smoke-Test war
sample-identisch. Die nativen FLAC-/Opus-Tests sind auf diesem Host erwartbar
nicht ausführbar, weil die mitgelieferten Codecarchive `x86_64-Linux` sind und
der Build sie auf Darwin gezielt deaktiviert. Der gleichwertige Docker-Oracle
`iamf-tools:v2.1.0` lief dagegen erfolgreich: der CI-Vektor
`test_000003.iamf` meldete exakt 63 Temporal Units und erzeugte eine
64,080-Byte-WAV; separate AAC-LC-, FLAC- und Opus-Vektoren meldeten jeweils
positive Unit-Zahlen und erzeugten WAV-Ausgaben. Das ist Codec- und
Host-Abdeckung, keine Behauptung eines implementierten Codec-Encoders oder
Renderers.

## Empfohlener Abschlussplan

1. **P1-Restgrenze klar halten:** Den Delivery-Validator als Pflicht-Eingangstor für Dateimuxer verwenden. Eine globale Decode-/Presentation-Zeitachse gehört weiterhin in Decoder/Renderer, nicht als nachträgliche Writer-Behauptung.
2. **P2 bei Produktbedarf erweitern:** Multi-Layer/Scalable, Projection-Ambisonics sowie Demixing-/Recon-Gain-Builder jeweils mit Profil- und Referenztests ergonomisch machen.
3. **Profilbelege ausbauen:** Die vorhandenen Kern-Regressionen bei Bedarf zu einer separaten, erschöpfenden Matrix mit gepinnten Referenzfällen ausbauen.
4. **Playback separat aufbauen:** §7 einschließlich animierter Parameter und binauralem/normativem OAR-Rendering in Decoder-/Renderer-Crates implementieren, nicht in iamf-rs.

## Fazit

Als IAMF-v1.1-Bitstreambibliothek ist der aktuelle Stand mehr als ein
OBU-Dumper: typisierte Descriptoren, deterministische IDs/Profile,
kontextgesicherte Parameterblöcke, LPCM/FLAC/Opus/AAC-LC-Codec-Configs und
gepinnt geprüfte externe Orakel bilden eine gute Basis. Der explizite
Delivery-/Timeline-Layer schließt die wichtige Grenze zwischen
Streaming-Fragmenten und vollständigen Liefersequenzen. Nicht erfüllt und
nicht behauptet sind Decoder, Renderer, ISO-BMFF-Package-Erzeugung, globale
Presentation-Zeitachsenbeweise sowie jede ergonomische Spezial-Authoring-API.
