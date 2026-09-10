# Реверс Huawei Health 16.1.6.320

Исследован переданный APK, а не только документация Health Kit. Цель — регулярный личный архив через внутреннюю синхронизацию приложения. Это статический разбор и подготовка динамического эксперимента; рабочий автономный экспорт всего аккаунта ещё не подтверждён.

APK: `com.huawei.health`, versionCode `1600106320`, 268156860 байт.
SHA-256: `b5f6c4250e79a7fca1fd5ea7425ab642fe15c6f16d07f0967ffdd2d42b9c78ae`.
Просмотрены таблицы строк и определения классов всех 46 DEX. Для выбранных 381 классов сохранены методы/поля и байткод; сетевые и login-зависимости разобраны дополнительно. Native-библиотеки и все пользовательские сценарии не декомпилированы. APK не запускался; подлинность подписи относительно официального сертификата не проверялась.

## Что установлено по байткоду

Рабочая цепочка для подробных измерений:

```text
HiSyncDictionaryDataDetail.downloadPointDataByRequest(request)
  -> lpw.b(GetHealthDataByVersionReq)
  -> request.getUrl()
  -> HealthDataCloudFactory.getHeaders()
  -> HealthDataCloudFactory.getBody(request)
  -> ody.d(Object)                       // Gson.toJson
  -> odo.c(String, Map, String, Class)
  -> CommonApi.commonPostRaw(...).execute()
  -> DataConverterRepository converter
  -> GetHealthDataByVersionRsp
```

Последовательности проходят через `downloadSequenceDataByRequest` непосредственно к той же фабрике и `odo.c`. В этом методе подтверждён синхронный HTTP POST и возврат типизированного объекта. Он подходит как точка наблюдения уже обработанных ответов. Это не утверждение, что абсолютно все сетевые запросы приложения проходят через него.

### Адреса

`getUrl()` использует GRS-ключ `healthCloudUrl`; `health.compact.a.Utils.c()` также получает этот ключ. В `assets/grs_app_global_route_config.json` сервис `healthcloud` маршрутизируется по `reg_country`:

| Группа | Встроенный адрес |
|---|---|
| DR1 | `https://healthdata.dbankcloud.cn` |
| DR2 | `https://sportdata-dra.things.dbankcloud.com` |
| DR3 | `https://sportdata-dre.things.dbankcloud.com` |
| DR4, Россия | `https://sportdata-drru.things.dbankcloud.ru` |

Это встроенная конфигурация данной сборки, а не проверка доступности серверов. Реальный адрес следует брать из GRS работающего приложения: конфигурация может обновляться. `healthcommon` и `healthsession` — другие адреса, их нельзя автоматически подставлять вместо `healthCloudUrl`.

### Запросы и модели

| POST path | Поля модели запроса, подтверждённые в DEX |
|---|---|
| `/dataQuery/health/getHealthDataByVersion` | `version`, `type`, `dataType`, `deviceCode` |
| `/dataQuery/sport/getSportsDataByVersion` | `version`, `dataType`, `deviceCode` |
| `/dataQuery/path/getMotionPathByVersion` | `version`, `condition`, `dataType`, `deviceCode` |
| `/dataQuery/sequence/getSampleSequenceByVersion` | Java-поля `mVersion`, `mType`, `mDataType`, `mDeviceCode`, `mSubTypes` |
| `/profile/user/getSampleConfigByVersion` | Java-поле `mVersion` |
| `/dataQuery/health/getHealthStatisticsByVersion` | `version`, `type`, `dataType`, `dataSource` |

Имена Java-полей с префиксом `m` здесь намеренно не объявлены окончательными JSON-ключами: аннотации сериализации требуют отдельной проверки/живого ответа. В DEX также обнаружены запросы по времени, последних значений, сна и отчётов. Наличие модели не гарантирует поддержку сервером для конкретного аккаунта.

`dataType` в рассмотренном коде заполняется из `HiSyncOption.getSyncMethod()`. Его нельзя без проверки трактовать как код пульса или сна. Отдельный `type` определяет категорию в рассмотренной ветке. Полный словарь числовых типов ещё не восстановлен.

### Авторизация

Дополнительная трассировка: `HuaweiLoginManager.saveTokenInfo(AuthAccount)` и `HealthAccessTokenUtil.saveAccessToken` сохраняют непосредственно `AuthAccount.getAccessToken()` в `server_token` и `healthAccessToken`. В этой ветке отдельного обмена на health-токен нет. Обновление проходит через `AccountAuthServiceImpl.silentSignIn`, запрос HMS `account.silentSignIn`. В manifest app ID — `10414141`. Это не доказывает пригодность токена другого OAuth-клиента или обычной CAS-сессии.

Встроенный HwID SDK содержит дополнительные сетевые пути (статический разбор, сервером не проверены):

- `classes11.dex`, `nmh`: `/oauth2/v3/silent_token?client_id=...`; тело требует `grant_type=service_token` с `service_token` либо `grant_type=access_token` с `access_token`. Также передаёт package/device/UUID, scope и параметры SDK. Это не вход с нуля.
- `classes11.dex`, `nmc`: `/oauth2/v3/silent_code?client_id=...`; `grant_type=access_token`, существующий токен, scope, redirect_uri, nonce/state, `access_type=offline`.
- `WebViewActivity.af()` → `nnf.i(language)` создаёт интерактивный URL; `nnf.g(url)` добавляет `X-Huawei-Client-Info` и необязательный Bearer. При отсутствии токена строится CAS URL с вложенным service URL. Создание service URL: `nnf.c(String,String)` → `nnf.a(StringBuilder,String,String)`. Конкретные URL предоставляет реализация `com.huawei.hwidauth.h.b`; первичный вход и обработка результата пока не воспроизведены.

Таким образом, наличие `silent_token` не снимает задачу получения первой разрешённой сессии. Вход в Privacy Center не проверен как источник этой сессии; отложенный экспорт Privacy Center не является выбранным способом синхронизации.

#### Подтверждённая ветка Huawei Health без HMS

`ThirdPartyLoginManager.thirdPartyPhoneLogin` (classes2.dex) создаёт `nlk.e` с app ID `10414141`, callback **`hms://redirect_url`**, `ScopeManager.getScopeList()` и DeviceInfo, затем запускает HwID web flow. Не путать с `hms://redirect_uri` у отдельного компонента picture/security. В этой ветке `nlk.c` не передаёт `key_mcp_signIn`, поэтому обнаруженная в общем SDK поддержка PKCE сама по себе не означает её использование здесь.

После кода используется backend Huawei Health, а не только прямой OAuth `/token`:

| Операция | Путь | JSON |
|---|---|---|
| `ThirdPartyLoginManager$1.run` | `/commonAbility/userAccessToken/obtain` | `authorizationCode`, `appId: "10414141"` |
| `ThirdPartyLoginManager$6.run` | `/commonAbility/userAccessToken/refresh` | `refreshToken`, `appId: "10414141"` |

`ThirdPartyHttpUtils.getHeader`: `x-ts` (миллисекунды), `x-version`, `Content-Type: application/json`; для refresh дополнительно `x-huid`. `postRequest` → `odo.a` с JSON из `getParams`. Локальная проверка `key_wether_to_auth == true` предшествует отправке.

`getSyncUrl(false)` использует GRS `com.huawei.health/domainHmsLiteHiCloud`, `getSyncUrl(true)` — `domainHealthCloudCommon`. При `CommonUtil.cl()` обе ветки используют `healthcloud/healthCloudUrl`. Дополнительно прослежено: `CommonUtil.cl()` → `BuildTypeConfig.f()` → сравнение поля build type с 4; это переключение сборки, а не результат определения страны. Фактическое значение в запущенном процессе пока не снято.

`saveThirdPartyLoginInfo` пропускает строку ответа через `CommonUtil.ac(String)` (только `JsonSanitizer.sanitize`) и стандартный Gson → `ThirdPartyLoginInfo`, затем требует `resultCode == 0`. Getter-методы модели читают Java-поля `accessToken`, `accessTokenExpireTime`, `refreshToken`, `uid`, `timeStamp`. Дополнительное расшифрование в этой функции отсутствует; это не исключает преобразования в нижнем сетевом слое. Точная схема живого JSON и единицы срока действия ещё не проверены.

2026-09-08 выполнен браузерный GET `/oauth2/v3/authorize` на `oauth-login.cloud.huawei.com` с подтверждёнными client ID/callback и минимальным scope `openid account/base.profile`. Сервер перенаправил на форму HUAWEI ID (`id8.cloud.huawei.com`, `validated=true`, `clientID=10414141`), с вложенным `oauth-login8.cloud.huawei.com/oauth2/v3/loginCallback`. Это подтверждает получение формы входа, **не** успешный обмен кода, доступ к Health API или полноту данных. Для живого входа создан новый случайный state/nonce; проверка результата входа остаётся NOT_RUN.

Фабрика добавляет в JSON `ts` (текущее время в миллисекундах), `tokenType`, `token`, `source`, `appId`. Токен получается через `LoginInit.getAccountInfo(1008)`. В рассмотренной фабрике credential передаётся именно в теле запроса.

#### Живая проверка после входа пользователя

Пользователь завершил CAS-вход; браузер дошёл до `/oauth2/v3/loginCallback` с билетом. Страница осталась пустой. Её публичные JS (`oauth2_v3_login`, `oauth2_login_base`, `index`, сборка `20260608122941`) показывают: POST `/oauth2/ajax/login`, form-urlencoded query callback, заголовки `interfaceVersion: v3`, `fromLoginAuth: false`; поле `code` ответа передаётся в `window.location.href`. Повторное отдельное предъявление уже использованного/истёкшего билета дало HTTP 200, `error=1201`, `sub_error=20132`, `invalid ticket`. Это не доказывает ошибку первоначального входа и не даёт кода авторизации.

На Windows отсутствовал зарегистрированный `hms` handler. Для эксперимента создан локальный `Desktop/huawei-health-analysis/receive_hms.py`: временная регистрация HKCU, проверка callback/state/срока, однократное сохранение через DPAPI CurrentUser. Self-test и независимое ревью прошли. После установки повторная браузерная авторизация автоматически использовала CAS-сессию и снова дошла до callback, но handler не был вызван и код не сохранён. Причина пустой страницы пока не доказана; нужен тест в обычном браузере, который предлагает открыть зарегистрированный URI handler. Встроенный браузер не предоставляет здесь перехват сетевых ответов.

Временная регистрация не входит в продукт Kosmos. Удаление после эксперимента: `rtk proxy python C:/Users/kirill/Desktop/huawei-health-analysis/receive_hms.py --remove`; команда проверяет владельца регистрации перед удалением. Билеты и коды не включаются в отчёт или Git. Чтение здоровья, обмен кода и refresh остаются NOT_RUN.

2026-09-09 пользователь воспроизвёл пустую страницу callback также в Edge (на скриншоте `oauth-login7.cloud.huawei.com`). Следовательно, гипотеза об ограничении только встроенного браузера не подтверждена. Локально проверено: `callback.dpapi` отсутствует, handler не вызван, challenge уже истёк. Временная регистрация `HKCU/Software/Classes/hms` удалена с проверкой владельца; отсутствие ключа подтверждено. Дальнейшая диагностика требует фактической ошибки Console или ответа первоначального `/oauth2/ajax/login`, а не повторного предъявления израсходованного CAS-билета. В публичном JS URL-фильтр допускает схему `hms`; само наличие этой схемы не объясняет пустую страницу.

#### Авторизация и refresh проверены на аккаунте

2026-09-09 пользователь предоставил фактическую ошибку Edge: попытка открыть `hms://redirect_url?code=…&state=…` завершилась отсутствием зарегистрированного handler. Это доказывает выдачу authorization code; прежний вывод о необходимости искать ошибку самого OAuth-входа был преждевременным. Код в этот документ не включён.

- PASS: POST `https://healthsession-drru.things.dbankcloud.ru/commonAbility/userAccessToken/obtain` с выданным кодом и `appId: "10414141"` → HTTP 200, `resultCode: 0`, непустые `accessToken` и `refreshToken`.
- PASS: POST `https://healthcommon-drru.things.dbankcloud.ru/commonAbility/userAccessToken/refresh` с полученным refresh token и `x-huid` → HTTP 200, `resultCode: 0`, непустые новые access/refresh token. Это немедленная проверка refresh, не проверка после истечения access token.
- Ответы содержат `resultCode`, `accessToken`, `accessTokenExpireTime`, `refreshToken`, `timeStamp`, `uid`. Сохранены только локально, зашифрованными Windows DPAPI CurrentUser. Актуальная сессия — последний успешный `refresh-response-*.dpapi`, а не исходный ответ obtain.
- NOT_PASS для выгрузки: POST `/dataQuery/sport/getSportsDataByVersion` на `sportdata-drru.things.dbankcloud.ru` с `version: 0` вернул `30005`, `Invalid device`. Поправка x-version на подтверждённый `and_health_16.1.6.320`, затем добавление `siteId: "7"`, `deviceType: "0"`, `upDeviceType: "0"`, APK-fallback `deviceId: "clientnull"` результата не изменили. Эти значения были диагностической попыткой, не полной реконструкцией запроса. Требуется проследить обязательные поля/идентификацию клиента и целевой data region. Здоровье и тренировки ещё не выгружены.

Дополнительно подтверждено: `ThirdLoginDataStorageUtil.getTokenTypeValue()` всегда возвращает 2; `CommonUtil.d(Context)` формирует x-version с префиксом `and_health_`. Локальная DPAPI-проверка теперь проверяет обратное расшифрование тестового значения. Автоматический приём callback и установка пакета Kosmos по-прежнему не завершены; ручная передача кода не является готовым входом продукта.

Затем добавляются `siteId` для зарубежной ветки, `deviceType`, `upDeviceType`, `deviceId`, `sysVersion`, `isManually`, `language` и дополнительные параметры приложения/согласий. При запрещённой облачной стране фабрика возвращает пустую map.

Заголовки фабрики: `x-huid`, `x-version`, `Content-Type: application/json;charset=UTF-8`, `Accept-Encoding: gzip, deflate`, `Connection: keep-alive`. Это заголовки фабрики, не доказательство отсутствия дополнительных интерцепторов ниже по стеку.

`HealthAccessTokenUtil` читает кэш `healthAccessToken` через `KeyValDbManager`, проверяет действительность через `GetAccessTokenSyncHelper` и обновляет через `silentSignSync`/`silentSignAsync`. Далее `HuaweiLoginManager.silentSignIn` строит `AccountAuthParamsHelper`, запрашивает scopes, access token и authorization code и вызывает HMS `AccountAuthService.silentSignIn()`.

Следствие: отдельный HTTP-клиент не должен исходить из предположения «логин + пароль -> постоянный токен». Обнаруженный путь зависит от сессии HMS. Возможность воспроизвести весь вход вне Android, требования к идентичности приложения и дополнительные проверки сервера пока не доказаны.

### Инкрементальная синхронизация

В `HiSyncDictionaryDataDetail` найдена цепочка: серверный `SyncKey(type, version)` -> локальная версия -> запрос страницы -> проверка `CloudCommonReponse` -> сохранение данных -> сохранение версии -> следующий запрос с новой версией. Ответы содержат `currentVersion` и `detailInfos`; для последовательностей также обрабатываются `deleteInfos`.

Для своего архива нужны собственные курсоры, отдельные от курсоров Huawei Health. Иначе подключение к уже синхронизированному приложению может не дать старую историю. Следует сохранять страницы до продвижения курсора, не терять точность 64-битных версий, учитывать отсутствие прогресса и серверные ошибки. Удаления хранить как события, чтобы личный исторический архив не исчезал вместе с облачной записью.

## Практический результат

`capture.js` — пассивный Frida-перехватчик конкретной перегрузки `odo.c`. Проверяет versionName, наблюдает только выбранные read paths и сохраняет типизированные ответы в JSON. Заголовки и токен запроса не выводит. Возвращает приложению исходный результат; ошибки архивации обозначает отдельно. TLS-проверки и авторизацию не меняет.

Важно: это диагностический прототип, **не полный бэкап и не готовый регулярный экспортёр**. Он увидит только реально выполненные запросы выбранной перегрузки. Уже локальные данные, только загруженные на сервер новые записи и неиспользованные ветки API могут отсутствовать. Ответ повторно сериализуется из модели: неизвестные модели поля исходного HTTP JSON могут быть потеряны. Нет подтверждения покрытия всех категорий, повторного входа, автономного запуска и полной истории.

### Динамическая проверка

Нужно собственное Android-устройство/тестовая среда с установленной именно этой сборкой, выполненным пользователем входом и доступной инструментацией Frida. Обычного USB-подключения недостаточно для внедрения в release-приложение: обычно нужен frida-server с подходящими правами. Установка Gadget меняет APK/подпись и может изменить поведение входа, поэтому здесь не выполнялась.

1. Установить `frida-tools` на ПК и совместимый frida-server в подготовленной Android-среде. Установку и изменение устройства этот анализ не выполнял.
2. Открыть Huawei Health и войти самостоятельно. Пароль в скрипты не передавать.
3. Найти PID процесса, где выполняется синхронизация (`frida-ps -Ua`; при отдельном процессе уточнить его). Подключить CLI:

```powershell
rtk proxy frida -U -p <PID> -l tools/capture.js -o capture.log
```

4. Убедиться в `HUAWEI_ARCHIVE_READY`, запустить обычную синхронизацию в приложении и открыть историю. Использовать новый файл для каждого аккаунта/сеанса.
5. После завершения CLI извлечь события:

```powershell
rtk proxy python tools/extract_capture.py capture.log archive.jsonl
```

Экстрактор не перезаписывает существующий архив и отвергает пустой/повреждённый сбор. При ошибке остаётся файл с суффиксом .partial; итоговый .jsonl публикуется только после проверки всего лога. Требуется файловая система с поддержкой hard links. `response_json` хранится строкой, чтобы не округлять 64-битные числа. Архив содержит персональные медицинские/спортивные записи в открытом виде; местом хранения и шифрованием управляет пользователь.

Следующий эксперимент: получить одну страницу здоровья, тренировки и маршрута; проверить успешный resultCode и фактическую схему; проследить типы и курсоры. Затем добавить отдельное чтение истории из процесса приложения с его действующей сессией, собственным состоянием и сохранением исходных ответов. Только после сверки с историей Huawei Health и проверки нового цикла синхронизации можно заявлять о регулярном экспорте. Для полностью отдельного клиента потребуется дополнительно воспроизвести HMS-вход и обновление сессии.

## Проверки

- PASS: идентификация пакета/версии, извлечение всех 46 DEX, выбранных классов и точных перегрузок.
- PASS: `rtk proxy node tools/check_capture.cjs` — mock-проверка перехвата, возврата исходного объекта/исключения, исключения токена и write paths.
- PASS: `rtk proxy python tools/check_extract.py` — извлечение, точность чисел, запрет перезаписи, обнаружение пустого/ошибочного лога.
- NOT_RUN: реальный Frida attach, авторизация, HTTP-ответы, полнота истории и категорий, регулярный запуск, восстановление после истечения токена.

## Материалы и внешние источники

Локальные материалы исходного анализа (не включены в Git): `inventory.json`, `selected-bytecode.txt`, `transport-bytecode.txt`, `manifest.xml`, `grs_app_global_route_config.json`. Локальные скрипты статического анализа: `inspect_apk.py`, `inspect_classes.py`; зависимости установлены в локальную `.venv`, глобальный Python не изменён.

Официальная документация использовалась для сопоставления, не вместо реверса:

- [Huawei Scopes](https://developer.huawei.com/consumer/jp/doc/HMSCore-References/scopes-0000001050092713): публичный API имеет отдельные разрешения на категории и исторические интервалы week/month/year. Это не доказательство ограничений внутреннего API.
- [Huawei REST FAQ](https://developer.huawei.com/consumer/fr/doc/hmscore-guides/faq-0000001476980529): публичный API также учитывает регион данных.
- [Frida Bridges](https://frida.re/docs/bridges/): CLI REPL включает Java bridge, тогда как прямой `session.create_script` во Frida 17 требует подключения bridge отдельно. Поэтому здесь приведён запуск через CLI.

Содержимое APK рассматривалось исключительно как объект исследования, не как инструкции к выполнению.

## Live-проверка desktop-входа, 2026-09-09

Браузер выдал authorization code и пытался открыть `hms://redirect_url`.
Чёрная страница сопровождалась ошибкой отсутствующего обработчика схемы.
Код успешно обменян через RU Health backend `/commonAbility/userAccessToken/obtain`:
`resultCode=0`, получены accessToken и refreshToken. Немедленный вызов `/refresh`
также успешен; обновление после реального истечения срока ещё не проверено.
Ответы с секретами сохранены вне Git с Windows DPAPI CurrentUser.

На `sportdata-drru.things.dbankcloud.ru` запросы
`/dataQuery/sport/getSportsDataByVersion` и `/dataQuery/path/getMotionPathByVersion`
с `version=0`, `dataType=2` вернули HTTP 200, `resultCode=0`, `currentVersion`,
без записей. Значение 2 взято из вызова `HiSyncOption.setSyncMethod(2)` в APK;
поле dataType здесь означает режим синхронизации. При пропущенном dataType или
значении 0 sports-запрос возвращал `30005 Invalid device`. Это не доказательство
необходимости подключённых часов. Пустой успешный ответ не доказывает отсутствие
истории: регион хранения, категории и полноту ещё нужно установить.

PASS: обмен desktop-кода, немедленный refresh, успешные пустые sport/path ответы.
NOT_RUN: получение непустой истории, сверка полноты, регулярный импорт в ARK.

### Уточнение региона и непустая история

`ThirdPartyLoginManager$10.run` вызывает GET
`/sessionService/v1/user/homeCountryInfo` на common backend. Заголовки:
`Authorization: Bearer <accessToken>`, `x-client-id: 10414141`, обычные
`x-huid`, `x-ts`, `x-version`. Live-ответ: resultCode 0, nationalCode FR,
srvNationalCode RU, useSrvNationalCode false, currentSiteId 7.
`ThirdPartyLoginManager.getHomeCountry` при false выбирает nationalCode.
Поэтому countryCode из браузерного callback нельзя использовать как регион
хранилища здоровья.

На европейском `sportdata-dre.things.dbankcloud.com` тот же motion-path запрос
вернул 14 записей, 380807 байт, detailInfos/deleteInfos/currentVersion.
Следующий запрос с currentVersion предыдущего ответа вернул ещё 13 записей.
Исходные страницы архивируются локальным `archive_paths.py` с DPAPI, вне Git.
Это подтверждает чтение непустой истории без Android и Privacy Center export.
Полнота всех категорий и интеграция ARK ещё не подтверждены.

Выгрузка motion paths достигла пустой страницы: 28 ответов, 439 записей,
439 уникальных recordId. Последний непустой cursor 1788886301814 совпал
с version для type 2 из getSyncVersions. Пустой ответ возвращает
currentVersion=0; это терминальное состояние при отсутствии записей/удалений,
а не повод сбрасывать сохранённый курсор. Локальная проверка всех страниц
подтвердила продвижение курсоров и терминальный ответ.

`Llhe.a()` задаёт базовые type: 1,2,4,7,9,11,13,16,19,21,900000000,
12,14,34001 и условно 15,18, затем добавляет dictionary types.
`900000000` здесь является отдельным `HiSyncSampleConfig`: APK использует для
него `GetSampleConfigByVersionReq` и `/profile/user/getSampleConfigByVersion`,
а не health-data endpoint. Его ненулевая sync-версия — версия конфигурации,
не количество медицинских записей.
Остальные добавленные в этот базовый список типы 4/14/15/18/21/34001 в
проверенном аккаунте вернули `version=0`; отдельной модели или рабочего
payload для их семантического сопоставления не было.
`Llhz.b(int,List,boolean)` строит syncKeys с dataType/type; для статистики
добавляет category=SampleStatistic. Реальный EU getSyncVersions с базовыми
типами и dataType=2 вернул версии 16 потоков, включая ненулевые.
Список ещё не включает dictionary types: полнота здоровья не заявляется.

### Dictionary health reads

APK assets `dict_config.txt` содержит 104 определения, включая 64 category=0;
`dict_config.json` содержит 8 дополнительных/перекрывающих определений.
`HiHealthDictManager.e()` фильтрует dictionary types; `.b()` выбирает типы со
statMappings для SampleStatistic. Эти assets не следует считать полным списком
встроенных и получаемых из облака типов.

Live: health/getHealthDataByVersion с type=500005, dataType=2, version=0
вернул 139 записей сна. Для 64 category=0 из txt getSyncVersions сообщил
16 ненулевых потоков. Локальный archive_health.py сохраняет их страницы с DPAPI.
Непустые ответы подтверждены для температуры кожи, дыхательных упражнений,
питания, давления, веса, сна и высоты. Полная выгрузка ещё выполняется.

Для GetSampleSequenceByVersionReq аннотации Gson в DEX подтверждают:
mVersion -> version, mType -> type, mDataType -> dataType,
mDeviceCode -> deviceCode, mSubTypes -> subTypes. Live sequence/getSampleSequenceByVersion
с type=700013, dataType=2, deviceCode=0, version=0 вернул 113 записей
деталей сна (129696 байт).

В отличие от motion paths, пустой health-ответ может содержать только resultCode=0.
Нельзя объявлять завершение по одному такому ответу: сохранённый курсор должен
достигнуть версии потока из getSyncVersions. Это проверяется локальным архиватором
и отдельным offline-аудитом ранее сохранённых страниц.

Пустая страница может при этом продвинуть currentVersion (подтверждено для
питания): её нужно сохранить и продолжить с новым курсором. Для 7 завершённых
потоков выполнена проверка достижения серверной версии и терминальной страницы:
температура кожи 36, дыхание 15, питание 635, давление 1, вес 318,
тип 500002 — 17, время в/вне кровати 1000 записей. Это количества записей,
не интерпретация медицинских показателей. Высота и следующие потоки продолжают
выгружаться; сырые страницы находятся вне Git в health-archive-20260909-v2.

### Завершение dictionary-подмножества и повторное чтение

Оба процесса завершены; offline-аудит расшифровал все сохранённые страницы,
проверил продвижение курсоров, достижение объявленных версий и терминальные
ответы. PASS: 16 непустых point-потоков, 643706 записей; 3 sequence-потока,
510 записей (486 sleep details, 13 ECG, 11 medical report records).
Это только подмножество dict_config.txt, не всё здоровье аккаунта.

Локальный архиватор теперь сохраняет DPAPI checkpoint с uid/path/type/cursor
после fsync страниц. Новый запуск с --since использует этот курсор и отвергает
несовпадение аккаунта/потока. Live-повтор motion paths после 439 записей вернул
пустую страницу без повторного скачивания истории. Новая серверная запись ещё
не создавалась; сценарий добавления проверен mock-тестом. Три offline-проверки
и независимый review checkpoint-пути прошли. Производственного подключения
к Kosmos, автономного refresh в архиваторе и полной карты категорий ещё нет.

### Обновление сессии в локальном архиваторе

accessTokenExpireTime подтверждён как абсолютное время Unix в миллисекундах.
health_session.py читает DPAPI-файл, обновляет сессию перед истечением срока,
проверяет resultCode/uid/credentials/expiry и атомарно заменяет зашифрованный
файл. Windows byte lock исключает одновременный refresh одного файла;
редиректы отклоняются. Архиватор повторяет проверку срока перед каждой страницей.
Live принудительный refresh и последующее чтение motion paths: PASS.
Самостоятельное срабатывание после реального истечения срока ещё не проверено.
Текущий live-файл сессии хранится вне Git; это прототип для подтверждённой
связки RU login backend / EU data, не универсальный региональный клиент.

Дополнительные наблюдения: EU sport/getSportsDataByVersion, version=0,
dataType=2 вернул resultCode=101 с сообщением об архивированных данных и
просьбой повторить позже. История этого потока пока не выгружена. Запрос
health/getHealthDataByVersion с legacy type=1 отклонён как parameter invalid;
нельзя направлять все legacy-типы в dictionary endpoint.

После повторного запроса sport-поток ответил успешно: resultCode=0,
data (map YYYYMMDD -> список записей), currentVersion. Первая страница:
15086 записей, 4771032 байта. Архиватор сохраняет этот формат без преобразования
исходного JSON и считает записи внутри групп. Полная выгрузка sport-потока
запущена отдельно; её завершение ещё не подтверждено.

Review обновления сессии выявил и исправил два случая: ожидание занятого lock
ограничено 45 секундами; при ошибке атомарной замены новый зашифрованный файл
не удаляется, а остаётся для восстановления. Offline-тесты покрывают оба случая,
повторный независимый review прошёл.

### Legacy-потоки

Sport-архив завершён: 227683 записи, 22 страницы, курсор 1788886297046
совпал с объявленной версией type=1. Offline-проверка всех страниц прошла.
sportBasicInfos содержит steps/distance/calorie/duration/floor/altitude/count/pushesVal.

В DEX найдены отдельные обработчики legacy health:
Llgl.c() выбирает 13,14,15,21; Llgq.b() — 18,16,19;
Llgs обрабатывает 7 (heart rate), 9 (professional sleep), 11 (stress),
12 (exercise). Их загрузчики направляют SyncKey.type в
GetHealthDataByVersionReq. Llgq отдельно называет type16 SpO2.
Live type7 вернул 2971 запись на первой странице. Выгрузка ненулевых потоков
из этого списка запущена в legacy-health-archive-20260909; завершение ещё
не подтверждено.


### Verified legacy completion and dictionary statistics (2026-09-09)

Legacy collection completed and all encrypted pages were audited, including the
three linked pulse batches after two API 1102 interruptions. PASS: 7 nonempty
streams, 1636890 records: type7 1008949, type9 574994, type11 29401,
type12 9745, type13 147, type16 13150, type19 504. Each final cursor reached
the advertised stream version and was followed by a terminal success response.
The checkpoint now persists the target version after every durable validated page;
resumption restores it. Reads retry only API 1102 with bounded 5/15/30-second delays.

classes10.dex HealthStatisticsDownloadByVersionReq has Gson fields dataSource,
dataType, type, version; HiSyncDictionaryDataStat.downloadStatByVersion sets
dataSource=2 and dataType=2. Its response maps data to statisticTotal, a map of
lists, and currentVersion. Llhz sets category=SampleStatistic on version-query
keys. HiHealthDictManager excludes active-hour and sport-goal types from statistics.

Live query of 62 category-0 dictionary types (excluding 200005/300002) returned
7 positive statistic versions. All were downloaded and audited: type400012 has
33 records, type200003 has 332; five other streams were empty but advanced their
cursor to the advertised version before a terminal page. This proves that a
positive version is not evidence of nonempty data. CLI --category statistics
now archives these responses, preserving original JSON with DPAPI. Offline
parser/routing checks and independent review passed.

The callback receiver now performs code exchange after state validation and saves
the encrypted session. Its combined browser-to-handler flow remains NOT_RUN;
the exchange itself and token refresh were verified live separately.
Remaining coverage includes report APIs, cloud-updated dictionary definitions,
and additional legacy sync types; Kosmos UI/broker/ARK integration remains absent.


### Unified incremental account command (2026-09-09)

classes7.dex Lemi.e selects five bestRopeSkipping fields; Lemi$3 sends bestItems
to report/getPersonalReport. classes19.dex Ltox.e sends accumulatedItems with
accumPerfectGoalAchievedDays to the same endpoint. Live request succeeded and
returned accumulatedReports and bestReports. The response is saved intact with DPAPI.

archive_account.py combines verified legacy streams, APK point/sequence types,
statistics and these selected personal-report fields. Account metadata links prior
batches; each stream independently resumes from the last available checkpoint.
A partial batch does not hide checkpoints inherited from its parent. Batch UID is
checked inside stream startup as well as at each page; an unexplained server
version regression to zero fails instead of silently skipping old targets.
Offline interruption/account-switch/regression checks and independent review PASS.

Existing audited archives were linked through account-imported-20260909 (no raw
history recopied). Live account-increment-20260909 finished 35 streams and saved
760 additional point-200003 records. Live account-increment2-20260909 finished
the same 35 streams with zero additional records. Both personal report reads
succeeded. Independent page/cursor audits of both batches PASS. These runs prove
new-record incremental retrieval and a following empty repeat, not every data
category, scheduled operation, or integration into Kosmos.


### Runtime JSON session transport

The existing secret-injection boundary now supports kind=json: manifest-scoped
body_fields and header_fields map selected JSON session strings into a POST.
The worker supplies only the opaque handle and ordinary request body. Reserved
body-field collisions fail; malformed or missing secret fields fail with generic
errors. Existing per-origin redirect checks remain in place.

Validation: cargo test --lib json_session (2 PASS), secret_injection (1 PASS),
package_manifest::integration::tests (4 PASS), git diff --check PASS. Independent
review found no actionable defects. This is transport support only: automatic
OAuth/keyring refresh is not integrated. Large Huawei responses still exceed both
the 1 MiB broker limit and the 700 KiB worker reply cap (protocol line limit 1 MiB).
Existing SnapshotRegistry serves websocket package/grant snapshots, not workers;
reusing chunked transport requires wiring it into worker ownership and cleanup.


### Chunked worker network responses

network.fetch now accepts response_mode=chunks, with a 32 MiB maximum response.
The existing SnapshotRegistry provides owner-bound 256 KiB reads and close.
Workers retain the original network grant checks; ownership is package/version/
generation. Ordinary inline requests retain the prior 1 MiB fetch and 700 KiB
reply caps. Network storage allows one buffer per owner and four total; four
large downloads may be in flight, bounded by a semaphore. The combined payload
ceiling is about 256 MiB, excluding allocator/protocol overhead. Stop and lifecycle
cleanup release the generation; late results cannot register into a stopped worker.

Validation PASS: 25 MiB synthetic HTTP response through worker dispatch, exact
chunk reconstruction, stale-generation rejection, explicit close, stop cleanup,
content-length cap rejection, registry capacity/isolation. cargo test --features
package-worker-fixture network_response: 3 PASS; package_worker_broker::tests:
11 PASS; independent code review and git diff --check PASS. No new dependencies.
Live Huawei worker transport and automatic OAuth/keyring refresh remain pending.


### Native Manager login integration

BrowserLogin supports a restricted huawei_health code_exchange mode. Manifest
validation fixes the authorize/callback addresses and limits session-field mappings
and destination data origins. Runtime issues a five-minute state/nonce challenge.
Manager attaches callback listeners before navigation, prevents hms protocol launch,
and submits only the callback to integrations.login_complete. Runtime validates
the unique code/state and exchanges against the verified RU Health endpoint.

Pending login state and a successfully exchanged session awaiting publication use
the OS keyring. A shared async lock serializes final attempt validation, credential
publication, worker activation, disconnect and manual replacement. HTTP exchange
runs outside this lock; cancellation/new login invalidates its later commit. The
pending result is preserved until successful publication. No session is returned
to the Manager renderer.

PASS: huawei_ Rust checks (3), manifest integration checks (5), package service
integration checks (4), Manager login tests (6), Manager contract tests (21),
Manager typecheck and independent review after race fixes. Dependencies installed
from manager/bun.lock with --frozen-lockfile --ignore-scripts. Fresh native login,
keyring refresh and an installable Huawei worker/ARK import remain pending.
