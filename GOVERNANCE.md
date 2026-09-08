# PTIFF Governance

Dieses Dokument beschreibt, wie das PTIFF-Projekt — insbesondere die
Offene Spezifikation und die Referenzimplementierung (reiner Rust-Workspace) —
verwaltet wird.
Es ist der normreferentielle Bezug für Goal G4 ("Open specification and governance")
des RFC-0001 und legt die Prozesse fest, über die Änderungen an der Spezifikation
entschieden werden.

PTIFF verfolgt eine **offene, transparenter Governance**: jede Änderung an einem
normativen Dokument läuft über den RFC-Prozess unten; keine Einzelperson oder
Organisation ändert die Spezifikation unilateral.

## Kontakt

Kontakt-Kanäle werden derzeit **lokal** gepflegt. Ein öffentliches Repository und
dauerhafte Projekt-Kontaktadressen (z. B. ein Issue-/Diskussionskanal und eine
Sicherheits-Adresse) sind noch nicht festgelegt; sobald das Projekt publiziert wird,
werden die Kanäle hier ergänzt. Bis dahin wende dich bitte an die Projektleitung
über die jeweils aktiven lokalen Kanäle (persönlich/fachlich verfügbare Wege der
Maintainers).

- **Sicherheitsmeldungen:** siehe `SECURITY.md` (vertraulich, nicht öffentlich).
- **Verhaltenskodex-Meldungen:** über die in `CODE_OF_CONDUCT.md` genannten Kanäle,
  an die Projektleitung.

### Review-Kanal für den RFC-Prozess

Der kanonische Ort für öffentliche RFC-Reviews ist das GitHub-Repository:

```text
https://github.com/afeldman/ptiff
```

RFC-Reviews laufen über **GitHub Issues** in diesem Repository. Das zu einem
RFC gehörende Review-Issue ist das maßgebliche Review-Protokoll: es dient als
Ankündigung, Diskussionsprotokoll und Entscheidungsprotokoll. GitHub
Discussions sind für den RFC-Prozess nicht erforderlich.

## Status

Dieses Dokument ist **vorläufig**. Der Governance-Prozess wird so lange iteriert,
bis die Spezifikation den "Stable"-Status erreicht und eine breitere Gemeinschaft
(Agenturen wie NASA/PDS, Universitäten, Industrie) eingebunden ist.

## 1. Rollen

| Rolle | Verantwortung |
|-------|---------------|
| **Projektleitung (Maintainers)** | Verwaltet Repos, Merge-Entscheidungen, zielt auf Qualität, besetzt/durchläuft den RFC-Prozess. |
| **Mitwirkende (Contributors)** | Steuern Code, Tests, Doku und Design bei — unter der Apache-2.0-Lizenz. |
| **Nutzer (Users)** | Verwenden die Spezifikation/Implementierung und liefern Feedback und Anwendungsfälle. |

Grundsatz: Jede*r, der sich gemäß `CODE_OF_CONDUCT.md` verhält, kann beitragen.
Rollen sind verdient, nicht vererbt; die Projektleitung kann sich aus der
Gemeinschaft heraus erneuern.

### Projektleitung / Maintainer

| Rolle | Person |
|-------|--------|
| **Projektleitung / Maintainer** | Anton Feldmann |

- Die genannte Person vertritt die Projektleitung (Maintainer) des Projekts
  und übt die in §3 und §4 beschriebenen Entscheidungsbefugnisse aus.
- RFC-Approval-Entscheidungen (Statuswechsel `Draft`/`Ready for Approval` →
  `Approved`) trifft die Projektleitung / der Maintainer gemäß §4.
- In jedem Approval-Protokoll wird der entscheidende Maintainer namentlich
  genannt.
- Bloße Git-Autorenschaft begründet keine Governance-Befugnis; maßgeblich
  ist die Eintragung in dieser Liste.
- Änderungen an dieser Liste dokumentiert die Projektleitung selbst mit
  Datum und Begründung.
- Quoren und Abstimmungsgremien sind nicht vorgesehen; Entscheidungen
  folgen dem Konsensprinzip (§3) und im Bedarfsfall der offen begründeten
  Entscheidung der Projektleitung.

## 2. Lizenz

- **Spezifikation** (Dokumente unter `rfcs/`, `specification/`, ...): Apache-2.0
  gemäß `LICENSE-SPEC`. Die Spezifikation ist frei implementierbar, auch in
  proprietären und Closed-Source-Kontexten.
- **Implementierung** (Rust-Workspace: `ptiff-core` → `ptiff-rust` → `ptiff-cli`/`ptiff-c`):
  Apache-2.0 gemäß `LICENSE`. Beiträge werden
  unter denselben Bedingungen eingereicht (siehe auch Abschnitt "Beiträge" in
  `CONTRIBUTING.md`).

## 3. Entscheidungsfindung

- **Konsens-Prinzip.** Für alltägliche Änderungen reicht das Ermessen der
  Projektleitung. Für normative Spezifikations-Änderungen gilt der RFC-Prozess
  (unten).
- **Eskalation.** Bei Dissens versucht die Projektleitung zuerst Konsens herzustellen.
  Bleibt er aus, entscheidet die Projektleitung mit offener Begründung. Richtschnur
  ist immer, im Sinne der langfristigen Interoperabilität der Gemeinschaft zu handeln.

## 4. Der RFC-Prozess (normative Spezifikations-Änderungen)

Normative Änderungen — z. B. neue Extension-Domänen, Tag-Allokationen,
Feld-Erweiterungen, neue Konformitätsstufen — MUST über den RFC-Prozess laufen:

1. **RFC einreichen.** Ein neues Proposal wird als RFC-Dokument unter `rfcs/`
   abgelegt (Vorlage/Draft) und ein Issue zur Diskussion geöffnet.
2. **Diskussion (mind. 2 Wochen Review-Fenster).** Offene Kommentare, Änderungen.
   Das Fenster ZAHLT ab dem Zeitpunkt der Ankündigung.
3. **Statuswechsel Draft → Approved.** Die Projektleitung beschließt nach der
   Review; ein RFC ohne beauftragte ausreichende Diskussion wird zur Überarbeitung
   zurückgegeben.
4. **Referenzimplementierung & Konformität.** Ein mit "Approved" gezeichneter RFC
   wird typischerweise durch eine Referenzimplementierung und/oder
   Konformitätstests begleitet (informativ, nicht normativ — siehe RFC-0001 NG5).
5. **Stabilisierung.** Nach Marktreife/breiter Akzeptanz kann ein RFC von "Approved"
   auf "Stable" übergehen. "Stable" RFCs können nur durch einen neuen RFC geändert
   oder abgelöst werden.

Jeder RFC ist unabhängig versioniert; Änderungen an bestehenden RFCs laufen
rückwärtskompatibel (additiv) bzw. über einen neuen RFC, der einen alten abtrennt.

### Operative Ausführungsregeln (Klarstellungen)

Die folgenden Regeln präzisieren den Ablauf aus §4, damit Reviews
nachvollziehbar und auditierbar durchgeführt werden können. Sie ändern keine
bestehende Regel.

- **Review-Issue.** Jeder RFC erhält ein eigenes GitHub-Issue mit dem Titel
  `RFC review: RFC-XXXX (vX.Y.Z)` in dem unter „Kontakt“ bezeichneten
  Repository. Das Issue ist Ankündigung, Diskussionsprotokoll und
  Entscheidungsprotokoll in einem.
- **Review-Start.** Die Review-Frist beginnt, wenn die Projektleitung / der
  Maintainer das Review-Issue eröffnet. Der Zeitstempel der Issue-Eröffnung
  ist der maßgebliche Startzeitpunkt.
- **Review-Dauer.** Die Review-Frist beträgt mindestens 14 Kalendertage ab
  Eröffnung des Review-Issues. Das voraussichtliche Fristende wird im Issue
  festgehalten.
- **Verlängerungen.** Eine Verlängerung wird durch einen Kommentar des
  Maintainers mit neuem Enddatum und Begründung im Issue dokumentiert.
- **Review-Kommentare.** Normale Kommentare sind beratend und blockieren
  keine Entscheidung.
- **Formelle Einwände.** Ein formeller Einwand wird ausdrücklich mit
  `FORMAL OBJECTION:` markiert und benennt den konkreten Anlass sowie die
  betroffenen RFC-Inhalte. Vor einer Approbation muss jeder formelle Einwand
  aufgelöst sein durch: (1) Änderung des RFC, (2) Rückzug des Einwands oder
  (3) ausdrücklichen, schriftlich begründeten Maintainer-Override.
- **Approval-Entscheidung.** Nach Ablauf der Review-Frist dokumentiert der
  entscheidende Maintainer die Entscheidung im Review-Issue. Das Protokoll
  nennt: RFC, Version, Review-Beginn, Review-Ende, Einwände, Auflösung,
  Entscheidung, entscheidenden Maintainer und Entscheidungsdatum.
- **Normativer Status.** Nach einer Approbation wird `Status: Approved` im
  RFC selbst eingetragen; ein Revision-History-Eintrag nennt
  Approbationsdatum, entscheidenden Maintainer und Review-Zeitraum. Das
  Review-Issue bleibt das Belegprotokoll.
- **„Ready for Approval“.** Der Status `Ready for Approval` ist der
  Vor-Review-Status, den die Projektleitung setzt, bevor das formelle
  Review-Issue eröffnet wird. Weitere Statuswerte werden nicht eingeführt;
  die Statusvokabeln bleiben `Draft`, `Ready for Approval`, `Approved`,
  `Stable`.
- **RFC-Abhängigkeit.** Die Reihenfolge `RFC-0001 → RFC-7002` bleibt
  verbindlich: RFC-0001 muss approbiert sein, bevor RFC-7002 in sein
  Approbationsverfahren eintritt. Jeder RFC durchläuft sein eigenes
  Review-Issue und seine eigene Review-Frist; RFCs werden nicht in einem
  gemeinsamen Review zusammengefasst.

## 5. Verhaltenskodex & Durchsetzung

Es gilt der `CODE_OF_CONDUCT.md` (Contributor Covenant v2.1). Das Projektteam
nimmt Meldungen über die in der Doku definierten Kanäle entgegen. Beschwerden
werden vertraulich geprüft. Verstöße können zu Verwarnungen, vorübergehenden oder
dauerhaften Sperren führen; Konsequenzen werden von der Projektleitung festgelegt.

## 6. Release- & Versionspolitik

- PRe-1.0 gilt (SemVer): jede Veröffentlichung KANN die API/ABI brechen.
- Änderungen werden in `CHANGELOG.md` unter folgenden Versionen dokumentiert
  (Keep a Changelog). Versionsnummern aus der Workspace-Definition (`Cargo.toml`,
  `version.workspace = true`).
- Versionierung erfolgt im Rust-Kern über `ptiff::{APP_VERSION, VERSION_STR}` und
  (für die C-ABI) über die `ptiff_version_*`-Funktionen in `crates/ptiff-c`.

## 7. Umgang mit Dritt-Formaten

PTIFF verlässt sich auf konkurrierende/adjazente Standards (GeoTIFF, PDS3/PDS4,
ISIS, TIFF/BigTIFF selbst). Wo sich ein Extension-Domain mit einem etablierten
Standard überschneidet, SOLL er wiederverwenden statt parallel zu erfinden
(siehe RFC-0001 §11). Navigation zwischen Beteiligten obliegt der Projektleitung.

## 8. Leitplanken (Design- & Urheberrechts-Grundsätze)

- **Wissenschaftliche Integrität & Reproduzierbarkeit** sind Kernziele (RFC-0001).
- **Kein normatives Verhalten außerhalb der RFC-Prozesse.**
- **Transparente Entscheidungen**: Zu lange Diskussionen werden offen abgeschlossen.
- **Ein Projekt nicht an einen einzelnen Lieferanten binden**: Architektur hält den
  Exchange-Vorteil von PTIFF (offen, nicht vendor-locked).
