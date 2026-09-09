// Huawei Health 16.1.6.320 only. Load with the frida-tools REPL (-l).
// Passive: observes decoded read responses; does not initiate or modify requests.
'use strict';
const allowedPath = /^\/(?:dataQuery\/(?:health|sport|path|sequence|report)\/[A-Za-z0-9/]+|dataQuery\/common\/getSyncVersions|profile\/user\/getSampleConfigByVersion)$/;
const requestKeys = ['version', 'type', 'dataType', 'deviceCode', 'subTypes',
    'startTime', 'endTime', 'condition', 'dataSource', 'syncKeys'];

Java.perform(function () {
    const app = Java.use('com.huawei.haf.application.BaseApplication').d();
    const info = app.getPackageManager().getPackageInfo(app.getPackageName(), 0);
    if (String(info.versionName.value) !== '16.1.6.320') {
        throw new Error('Unsupported APK version; recheck bytecode before using this hook.');
    }
    const json = Java.use('org.json.JSONObject');
    const serialize = Java.use('ody').d.overload('java.lang.Object');
    const request = Java.use('odo').c.overload(
        'java.lang.String', 'java.util.Map', 'java.lang.String', 'java.lang.Class');
    request.implementation = function (url, headers, body, responseClass) {
        // Always preserve the original return value and exception behavior.
        const result = request.call(this, url, headers, body, responseClass);
        try {
            const path = String(url).replace(/^https:\/\/[^/]+/, '').split('?')[0];
            if (!allowedPath.test(path) || result === null) return result;
            const params = json.$new(String(body));
            const safeRequest = {};
            for (const key of requestKeys) {
                if (params.has(key)) safeRequest[key] = String(params.get(key));
            }
            const responseJson = String(serialize.call(Java.use('ody'), result));
            if (!responseJson) throw new Error('Empty serialized response');
            console.log('HUAWEI_ARCHIVE ' + JSON.stringify({
                schema: 1, captured_at: new Date().toISOString(), path,
                request: safeRequest, response_class: String(responseClass.getName()),
                response_json: responseJson
            }));
        } catch (_) {
            // Do not print exception text: it may contain a request body or health data.
            console.error('HUAWEI_ARCHIVE_ERROR capture failed; response not archived');
        }
        return result;
    };
    console.log('HUAWEI_ARCHIVE_READY passive read-response capture installed');
});
