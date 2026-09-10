# Huawei Health: проверка покрытия данных

Статус среза: **2026-09-10**.

Проверены текущие `getSyncVersions` по авторизованной сессии и локальные DPAPI-архивы запросов от 2026-09-09. В этом файле нет токенов и самих значений здоровья: только типы, статусы, количество записей и схема хранения.

## Итог

| Группа | Ответ версий | Ненулевые потоки | Сохранено записей | Статус |
|---|---:|---:|---:|---|
| Point / health | 64 из 64 | 16 | 643 706 | PASS |
| Sequence | 39 из 39 | 3 | 510 | PASS |
| Statistics | 62 из 62 | 7 | 365 | PASS, 5 потоков вернули 0 записей |
| Legacy health | 11 потоков | 7 | 1 636 890 | PASS для архивированных потоков |
| Sport | отдельный поток | — | 227 683 | PASS |
| Motion path | отдельный поток | — | 439 уникальных тренировок | PASS |

`version > 0` означает, что Huawei объявил поток доступным для аккаунта. `records = 0` означает, что ответ был успешно принят, но полезных записей в нём не было.

Текущий worker запрашивает 68 point-типов (64 из основного словаря + 4 подтверждённых extra), 63 statistics-типа (62 + `800003`) и 9 подтверждённых legacy-потоков. Неизвестные legacy-типы намеренно не включены в активный sync до выяснения их endpoint-контракта.

Важно: `legacy type 9` — это не «9 миллионов». Это идентификатор legacy-потока профессионального сна; в текущем архиве в нём 574 994 записи.

## Point / health

Все 64 типа вернулись в `getSyncVersions`. 16 ненулевых типов выгружены полностью:

| ID | Тип | Записей | Статус |
|---:|---|---:|---|
| 10002 | `BLOODPRESSURE` | 1 | PASS |
| 10006 | `WEIGHT_BODYFAT_BROAD` | 318 | PASS |
| 200003 | `ALTITUDE` | 624 041 | PASS |
| 200004 | `DRINK_WATER` | 2 | PASS |
| 200005 | `ACTIVE_HOUR` | 10 955 | PASS |
| 300002 | `SPORT_GOAL_ACHIEVEMENT_DATA` | 1 769 | PASS |
| 400012 | `SKIN_TEMPERATURE` | 36 | PASS |
| 400017 | `VASCULAR_HEALTH` | 14 | PASS |
| 500001 | `BREATH_TRAIN` | 15 | PASS |
| 500002 | `OBSTRUCTIVE_SLEEP_APNEA` | 17 | PASS |
| 500005 | `SLEEP_RECORD` | 2 310 | PASS |
| 500010 | `SLEEP_ON_OFF_BED_RECORD` | 1 000 | PASS |
| 500018 | `BASAL_METABOLISM` | 1 410 | PASS |
| 500019 | `CURRENT_BASAL_METABOLISM` | 390 | PASS |
| 500025 | `BASAL_METABOLISM_AFTER_EXERCISE` | 793 | PASS |
| 600002 | `DIET_RECORD` | 635 | PASS |

Типы, которые в текущем срезе имеют `version = 0`: `400011 BODY_TEMPERATURE`, `400013 ENVIRONMENT_TEMPERATURE`, `400014 HIGH_BODY_TEMPERATURE_ALARM`, `400015 LOW_BODY_TEMPERATURE_ALARM`, `500035 CONTINUE_BLOOD_SUGAR`, `500043 BLOOD_GLUCOSE_TREND`, `500036 URGENT_HYPOGLYCEMIA`, `500037 APPROACHING_HYPOGLYCEMIA`, `500038 HYPOGLYCEMIA`, `500039 HYPERGLYCEMIA`, `500040 BLOODGLUCOSE_RISE`, `500041 BLOODGLUCOSE_DECREASE`, `500045 BLOODPRESSURE_RISK_RESEARCH_RESULT`, `500046 BLOODPRESSURE_RISK_RESULT`, `500047 BLOODGLUCOSE_BLOODSUGAR`, `500031 EMOTION`, `500032 OVARY_HEALTH_DAILY_STATUS`, `500033 OVARY_HEALTH_RESULT`, `300014 MICRO_WORKOUT`, `400016 LOW_SKIN_TEMPERATURE_ALARM`, `400025 SUSPECTED_HIGH_TEMPERATURE_ALARM`, `400026 SUSPECTED_HIGH_TEMPERATURE`, `600001 LIGHT_FASTING`, `500027 FATTY_LIVER`, `500029 BLOOD_GLUCOSE_REMIND`, `600003 FASTING_LITE_PHASE`, `200001 MEDICATION_RULE`, `200002 MEDICATION_PUNCHING`, `500006 LAKE_LOUISE_AMS_SCORE`, `500007 ARRHYTHMIA_RESULT`, `400019 PHYSIOLOGICAL_CYCLE_BUSINESS`, `500034 SPORT_BLOOD_PRESSURE_RESULT`, `400023 VASCULAR_HEALTH_RESULT`, `500013 MONTHLY_SLEEP`, `500012 BG_DAILY_RESULT`, `500015 BG_DAILY_SLP_RESULT`, `500014 BG_RISK_GROUP_RESULT`, `400018 PHYSIOLOGICAL_CYCLE`, `400020 BODY_SHAPE`, `300003 DAILY_ACTIVITY_RECORD`, `300004 TODAY_ACTIVITY_RECORD`, `500030 BREATH_RATE`, `500044 HEART_RATE_VARIABILITY`, `500048 AVERAGE_WRIST_TEMPERATURE_ALL_NIGHT`, `500050 FOUR_LEAF_ACHIEVEMENT_DATA`, `500051 AFIB_BURDEN_RESULT`, `500052 RELAXATION`, `500055 BLOODGLUCOSE_REMINDERS`.

### Состав `WEIGHT_BODYFAT_BROAD`

В 318 записях реально обнаружены следующие поля:

| Поле | Непустых записей |
|---|---:|
| `bodyWeight` | 318 |
| `bodyFatRate` | 179 |
| `boneSalt` | 179 |
| `skeletalMusclelMass` | 179 |
| `muscleMass` | 99 |
| `bmi` | 179 |
| `bodyAge` | 179 |
| `bodyScore` | 179 |
| `bodyShape` | 178 |
| `basalMetabolism` | 179 |
| `moisture`, `moistureRate` | 179 / 179 |
| `visceralFatLevel` | 179 |
| `protein`, `proteinRate` | 62 / 80 |
| `left/rightArmFatMass` | 179 / 179 |
| `left/rightLegFatMass` | 179 / 179 |
| `trunkFatMass` | 179 |
| `left/rightArmMuscleMass` | 179 / 179 |
| `left/rightLegMuscleMass` | 179 / 179 |
| `trunkMuscleMass` | 179 |
| `heartRate` | 177 |
| `age`, `height`, `gender` | 199 / 199 / 199 |

Поля, объявленные APK, но отсутствующие в этом архиве: `healthyFatRate`, `healthyWeight`, `impedance`, `pressure`, `waistHipRatioUser`. Это отсутствие значения в данном потоке, а не ошибка запроса.

Для первой версии подключения `healthyFatRate`, `healthyWeight`, `impedance` и
другие справочные поля состава тела не являются обязательными полями read-model:
они остаются в исходной raw-странице, но не нужны для истории давления или
тренировок. Источник истины для давления — отдельный поток `10002`.

### Давление

`pressure` внутри `WEIGHT_BODYFAT_BROAD` в этом аккаунте не заполняется, поэтому это не история давления. История давления пришла отдельным point-потоком `10002 BLOODPRESSURE`: одна запись, sample key `BLOODPRESSURE`, поля `BLOOD_PRESSURE_SYSTOLIC`, `BLOOD_PRESSURE_DIASTOLIC`, `sphygmus` и `beforeMeasureActivity`. В APK этот поток назван `BLOOD_PRESSURE_SET`, то есть это правильный контракт для записей давления, внесённых через приложение или устройство.

`700017 MEDICAL_EXAM_REPORT` также содержит давление, но как часть медицинского отчёта: `systolicPressure`, `diastolicPressure`, `pressure` и `pressureAbnormalFlag`. Его нужно хранить как отчёт, а не смешивать с историей `10002`. Семантика числового `healthDataSource` не выдумывается: чтобы подтвердить ручной источник, нужен один новый ввод давления в Huawei Health и повторный запрос только `10002`.

Форма записи давления в raw-ответе подтверждена: `detailInfos[]` содержит
`recordId`, `startTime`, `endTime`, `healthDataSource` и `samplePoints[]`;
каждая точка содержит `key`, `value`, `fieldsMetadata` и `fieldsModifyTime`.
Значения в отчёт не копируются, raw-страница сохраняется целиком.

В уже сохранённой записи `10002` поле `healthDataSource` присутствует и имеет целочисленный код; в текущем срезе встретился код `2`. В APK найден только getter/setter без таблицы расшифровки, поэтому код фиксируем как исходную метаинформацию и не называем его «ручным» без контрольного ввода.

## Дополнительные типы из `dict_config.json`

В APK есть восемь типов, которых нет в основном `dict_config.txt`. Подтверждённые point-потоки 500021/500023/500024/500026 и statistics-поток 800003 теперь добавлены в worker и архивные скрипты. `200009` пока не добавлен: выбранные endpoint-параметры возвращают `1001`.

| ID | Имя | Версия | Проверка endpoint | Что реально видно | Покрытие |
|---:|---|---:|---|---|---|
| 500021 | `HIGH_HEART_RATE` | > 0 | `health/getHealthDataByVersion`: `0` | 276 записей, две непустые страницы и пустая терминальная; ключ `HIGH_HEART_RATE`; `maxHighHeartRateForAPeriod`, `minHighHeartRateForAPeriod`, `highHeartRateDetail`, `highThreshold` | PASS, полный extra-архив |
| 500022 | `LOW_HEART_RATE` | 0 | — | Данных нет; контракт содержит max/min/detail/threshold | PASS-empty |
| 200009 | `DAILY_ACTIVITIES` | > 0 | health/statistics/sequence: `1001`; sport endpoint ignores `type` | health-контракт не принят; sport вернул общие 15 086 записей, поэтому это не доказательство `200009` | Не выяснено |
| 500023 | `DYNAMIC_HEART_RATE` | > 0 | health endpoint: `0` | первая страница 2 919 записей; ключ `DYNAMIC_HEART_RATE`; поле `bpm` | Частичный probe |
| 500024 | `RESTING_HEART_RATE` | > 0 | health endpoint: `0` | 28 090 записей в двух связанных архивных частях; ключ `RESTING_HEART_RATE`; поля `oldRestBpm`, `restBpm` | PASS, полный архив через checkpoint |
| 500026 | `STRESS` | > 0 | health endpoint: `0` | первая страница 3 574 записи; ключ `STRESS`; поля `stressScore`, `stressDetail` | Частичный probe |
| 800003 | `SLEEP_PRO_RECORD` | > 0 | statistics endpoint: `0` | 644 дневных записей; поля `dayWakeupTime`, `dayFallAsleepTime`, `dayDuration`, `daySleepScore` и дыхательные/SpO₂ summary-поля | Probe PASS |
| 30001 | `SPORT` | > 0 | отдельный sport endpoint: `0` | спорт уже приходит через `sport/getSportsDataByVersion`; `type` там не фильтрует ответ | Покрыто sport-архивом |

`1001` здесь означает «этим выбранным endpoint/параметрами поток не читается», а не «данных точно нет». Дополнительный probe `sport/getSportsDataByVersion` вернул общую спортивную выдачу независимо от `type=200009`, поэтому считать её `DAILY_ACTIVITIES` нельзя. Матрица вариантов `dataType=0..10`, `deviceCode отсутствует/0/1` и `dataSource=0..3` для health/stats endpoint тоже не дала ни одного успешного ответа. В APK для 200009 есть только generic dictionary-поля `step` и `pushesVal`, отдельной модели или маршрута в разобранном DEX не найдено. Для `200009` нужен отдельный runtime capture из приложения; `800003` уже подтверждён через statistics endpoint. Для больших point-потоков выше зафиксирован только probe первой страницы, чтобы не тащить повторно многомегабайтную историю до определения правильного checkpoint.

## Sequence

| ID | Тип | Записей | Что видно в payload |
|---:|---|---:|---|
| 700009 | `ELECTROCARDIOGRAM` | 13 | средний пульс, arrhythmia flags, длина ECG и `detailData` waveform |
| 700013 | `SLEEP_DETAILS` | 486 | время засыпания/пробуждения, deep sleep, эффективность, score, дыхание, SpO₂ |
| 700017 | `MEDICAL_EXAM_REPORT` | 11 | вес, fat percentage, BMI, давление, SpO₂, skin temperature, average heart rate, sleep apnea и risk flags |

Остальные 36 sequence-типов в текущем срезе имеют `version = 0`: шум, course/golf, arrhythmia PPG, dynamic BP, RRI, research reports, glucose PPG, diving, breath-holding, one-second sport streams, mindfulness, ventilator, adventures, wheelchair, marathon, pickleball, merged running/bike, vascular PPG/ECG и respiratory-infection streams.

## Statistics

| ID | Тип | Записей | Статус |
|---:|---|---:|---|
| 200003 | `ALTITUDE` | 332 | PASS |
| 400012 | `SKIN_TEMPERATURE` | 33 | PASS |
| 400011 | `BODY_TEMPERATURE` | 0 | Пустой успешный ответ |
| 400013 | `ENVIRONMENT_TEMPERATURE` | 0 | Пустой успешный ответ |
| 400026 | `SUSPECTED_HIGH_TEMPERATURE` | 0 | Пустой успешный ответ |
| 500005 | `SLEEP_RECORD` | 0 | Пустой успешный ответ |
| 500031 | `EMOTION` | 0 | Пустой успешный ответ |

## Legacy и тренировки

| Поток | Записей | Примечание |
|---|---:|---|
| `sport/getSportsDataByVersion` | 227 683 | спортивная история |
| `path/getMotionPathByVersion` | 439 | 439 уникальных тренировок, 28 страниц |
| legacy type 7 | 1 008 949 | raw legacy stream |
| legacy type 9 | 574 994 | raw legacy stream |
| legacy type 11 | 29 401 | raw legacy stream |
| legacy type 12 | 9 745 | raw legacy stream |
| legacy type 13 | 147 | raw legacy stream |
| legacy type 16 | 13 150 | raw legacy stream |
| legacy type 19 | 504 | raw legacy stream |

Свежий инкрементальный probe от 2026-09-10 получил новые записи в legacy type 1/2/7/9/11/12/16. Потоки type 13 и 19 вернули пустой ответ. На `legacy-900000000` Huawei вернул `resultCode=1001`; этот поток нельзя считать рабочим без отдельного выяснения его endpoint/контракта.

### Расшифровка legacy type

| Type | Смысл | Наблюдаемые ключи |
|---:|---|---|
| 1 | спорт | `sportBasicInfos`: steps, distance, calorie, duration, floor, altitude, count, pushesVal |
| 2 | GPS/motion path | coordinate/location/track payload |
| 7 | динамический и покоящийся пульс | `DATA_POINT_DYNAMIC_HEARTRATE`, `DATA_POINT_NEW_REST_HEARTRATE` |
| 9 | профессиональный сон | `PROFESSIONAL_SLEEP_DEEP`, `DREAM`, `NOON`, `ON`, `SHALLOW`, `WAKE` |
| 11 | стресс | `STRESS_DATA`: score, grade, start/end time, algorithm/calibration flags |
| 12 | интенсивность упражнения | `EXERCISE_INTENSITY`: exercise type |
| 13 | тревога повышения пульса | `HEART_RATE_RISE_ALARM`: threshold, min/max heart rate, details |
| 16 | SpO₂ | `BLOOD_OXYGEN_SATURATION`: average saturation |
| 19 | напоминание о SpO₂ | `BLOOD_OXYGEN_REMIND`: reminder state |

Типы 4, 14, 15, 18, 21 и 34001 входят в APK-базовый список `Llhe.a()`, но не имеют отдельной модели/endpoint-ветки в разобранном DEX. В текущем аккаунте все шесть объявили `version = 0`, поэтому payload для семантического сопоставления отсутствует. Они исключены из активного sync-списка, чтобы один неподдержанный или пустой поток не блокировал остальные. `900000000` разобран отдельно ниже: это конфигурация sample-типа, а не поток медицинских записей.

### Что такое `900000000`

В APK этот идентификатор используется не как legacy-health запись, а как отдельный `HiSyncSampleConfig`. Для него приложение создаёт `GetSampleConfigByVersionReq` и вызывает `/profile/user/getSampleConfigByVersion`; ответ ожидается в `infoList`, а версия хранится как anchor конфигурации. Поэтому ненулевая версия в `getSyncVersions` не означает наличие измерений. На текущем аккаунте прямой запрос этого endpoint с каждым встроенным health-cloud origin вернул HTTP 404. `900000000` не включён в архив медицинских данных; возвращать его в обычный legacy sync нельзя.

## Как это будет храниться в ARK

Сейчас worker сохраняет исходную страницу ответа, а не теряет неизвестные поля при раннем преобразовании:

```json
{
  "object": {
    "id": "huawei:<uid>:point-10006:<cursor>:<sha256>:0",
    "typeId": "com.kosmos.huawei-health.archive",
    "contentJson": {
      "encoding": "base64",
      "bytes": "<raw Huawei response page>"
    },
    "propsJson": {
      "source": "huawei-health",
      "account": "<uid>",
      "stream": "point-10006",
      "cursor": 1786954579154,
      "sha256": "<digest>",
      "chunk": 0,
      "chunks": 1,
      "size": 123456
    }
  }
}
```

Поверх этого архива нужен нормализованный read-model для UI: `recordId`, `type`, `startTime`, `endTime`, `samplePoints` для point-данных и разобранные `summaryData/detailData` для sequence-данных. Исходную страницу оставляем как источник истины.

Минимальная форма нормализованной записи в Arc:

```json
{
  "id": "huawei:<uid>:<stream>:<recordId>",
  "typeId": "com.kosmos.huawei-health.measurement",
  "propsJson": {
    "source": "huawei-health",
    "account": "<uid>",
    "stream": "point-10002",
    "recordId": "<recordId>",
    "startTime": "<timestamp>",
    "endTime": "<timestamp>",
    "fields": [
      "BLOOD_PRESSURE_SYSTOLIC",
      "BLOOD_PRESSURE_DIASTOLIC",
      "sphygmus",
      "beforeMeasureActivity"
    ],
    "rawObjectId": "huawei:<uid>:point-10002:<cursor>:<sha256>:0",
    "schemaVersion": 1
  }
}
```

Значения остаются в нормализованном payload с типом и единицей измерения; `rawObjectId` связывает его с исходной страницей, поэтому неизвестные поля не теряются.

## TODO

- [x] Повторно проверить `getSyncVersions` для point/sequence/statistics.
- [x] Выгрузить все ненулевые point-потоки из текущего среза.
- [x] Выгрузить ненулевые sequence-потоки и проверить их payload.
- [x] Отдельно подтвердить вес, процент жира, костную массу, мышечную массу и сегментные показатели.
- [x] Проверить инкрементальный checkpoint на повторном запуске.
- [x] Проверить дополнительные активные типы из `dict_config.json` на первом ответе и зафиксировать их поля.
- [x] Подтвердить, что `legacy type 9` — профессиональный сон, а не отдельный «9-миллионный» поток.
- [x] Отделить историю давления `10002 BLOOD_PRESSURE_SET` от поля `pressure` в body composition и от aggregate medical report.
- [x] Разобрать `legacy-900000000`: это `HiSyncSampleConfig`, а не медицинский поток; endpoint `/profile/user/getSampleConfigByVersion` отдельно от health data.
- [x] Проверить legacy type 4/14/15/18/21/34001: это базовые `Llhe.a()` sync-ключи; у текущего аккаунта `version=0`, payload/model для них отсутствуют.
- [x] Полностью архивировать дополнительный поток 500021 `HIGH_HEART_RATE`.
- [ ] Полностью архивировать дополнительный поток 500023 `DYNAMIC_HEART_RATE`.
- [x] Возобновить и полностью архивировать дополнительный поток 500024 `RESTING_HEART_RATE` (28 090 записей в родительской и дочерней частях).
- [ ] Полностью архивировать дополнительный поток 500026 `STRESS`.
- [x] Найти statistics endpoint для `800003 SLEEP_PRO_RECORD` и подтвердить 644 записи.
- [ ] Найти endpoint для `200009 DAILY_ACTIVITIES`.
- [x] Добавить в worker подтверждённые 500021/500023/500024/500026 как point и 800003 как statistics.
- [ ] Добавить `200009 DAILY_ACTIVITIES` после нахождения его endpoint-контракта.
- [ ] Сопоставить legacy type 1/2/7/9/11/12/13/16/19 с нормализованными моделями.
- [ ] Провести контрольный ввод давления в Huawei Health и подтвердить источник `healthDataSource`.
- [x] Добавить schema/version для архивных ARK-объектов; нормализованные UI-объекты остаются следующим слоем.
- [ ] Сделать UI-представления для измерений, сна, тренировок и медицинских отчётов.
- [ ] Повторить тот же matrix-test через установленный native worker после публикации пакета.
