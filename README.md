# BattGuard

Utilitaire Windows (interface graphique) pour **prolonger l'autonomie et la duree de vie**
de la batterie d'un ordinateur portable.

![Plateforme](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)
![Rust](https://img.shields.io/badge/Rust-stable-orange)

## Fonctionnalites

- **Tableau de bord batterie** : pourcentage, etat (charge / secteur / batterie), rafraichi toutes les 2 s
- **Sante de la batterie** : capacite pleine vs capacite d'origine (rapport `powercfg /batteryreport`) avec verdict clair
- **Autonomie restante** : estimation Windows + estimation par observation de la tendance de decharge
- **Debit estime en watts** (calcule a partir de la tendance observee)
- **Profils d'alimentation en un clic** :
  - Equilibre / Econome / Performances
  - **Batterie prolongee** : profil Econome + processeur plafonne a 70 % sur batterie (CPU min 5 %)
- **Conseils pratiques** pour durer plus longtemps

Aucun driver, aucune modification risquee : uniquement les API officielles Windows
(`GetSystemPowerStatus`, `powercfg`).

## Installation / Build

### Depuis les artefacts GitHub Actions

1. Va dans l'onglet **Actions** du repo
2. Selecte le workflow **Build Windows EXE** > dernier run
3. Telecharge l'artefact `BattGuard-windows-x64` et decompresse-le

### Localement

```powershell
cargo run --release
```

Le binaire se trouve dans `target/release/battguard.exe`.

## Utilisation

- Lancez `battguard.exe`
- Sur batterie, cliquez sur **Batterie prolongee** (une demande d'administration peut s'afficher)
- La carte **Sante** affiche la degradation reelle de la batterie

## Notes

- Les modifications de plan d'alimentation demandent parfois les **droits administrateur** (UAC)
- Le seuil de charge 80 % depend du materiel/BIOS et n'est pas forcement supporte
- Windows peut marquer l'exe comme "inconnu" (SmartScreen) car il n'est pas signe : 
  **Plus d'informations > Executer quand meme**

## Licence

MIT
