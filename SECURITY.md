# Sicherheit

Sicherheitslücken zu melden trägt dazu bei, das Projekt für alle Beteiligten sicher
zu halten. Bitte melde Sicherheitsprobleme **nicht** über öffentliche Issues oder
PRs — melde sie vertraulich.

## Sicherheitsrelevante Bereiche

PTIFF verarbeitet pixelbasierte Bilddaten und deren wissenschaftliche Metadaten. Die
Parser behandeln alle Datei-Eingaben als **unvertrauenswürdigen Input** (siehe
RFC-0001 §13). Besonders kritisch:

- TIFF/BigTIFF-Parsing: IFD-Ketten, Offset/Längen-Felder, Tag-Anzahlen, Integer-
  Überläufe, übermäßige Speicherallokation aus manipulierten Feldern.
- Extension-Metadaten (camera, CRS, SPICE, ...): Offsets, Längen, Zählwerte,
  zyklische/selbstreferenzielle IFD-Offsets.
- Ressourcen-Erschöpfung durch große Felder/Strukturen.

Konforme Reader dürfen keine Metadatenfelder als Code ausführen (RFC-0001 §13).

## Ein Sicherheitsproblem melden

Sende eine vertrauliche Beschreibung an die Projektleitung über die derzeit
lokal gepflegten Kontakt-Kanäle (siehe `GOVERNANCE.md`; ein öffentlicher
Sicherheits-Advisory-Kanal wird ergänzt, sobald das Projekt publiziert ist).
Bitte nimm auf:

- betroffene Version(en),
- reproduzierenden Trigger (bevorzugt ein minimales Beispielfile / PoC),
- erwartete Auswirkung und Eingrenzung.

## Handhabung

1. Das Problem wird vertraulich bestätigt und geprüft.
2. Ein Fix wird erstellt und geclustert, zusammen mit einem Regressionstest.
3. Der Fix wird per gemeinsamem Security-Release verteilt, danach öffentlich
   dokumentiert (CVE-kompatible Beschreibung, falls anwendbar).
4. Reporter werden im Rahmen ihrer/en gewünschten Anerkennung gewürdigt.
