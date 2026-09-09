// One offline check of hook behavior. A mocked VM cannot validate Android/Frida.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
let hook, error, result = { currentVersion: '9223372036854775806', detailInfos: [] };
const logs = [];
const method = {call() { if (error) throw error; return result; }};
const java = {perform: fn => fn(), use(name) {
    if (name.endsWith('BaseApplication')) return {d: () => ({
        getPackageName: () => 'com.huawei.health',
        getPackageManager: () => ({getPackageInfo: () => ({versionName: {value: '16.1.6.320'}})})
    })};
    if (name === 'org.json.JSONObject') return {$new: text => {
        const data = JSON.parse(text);
        return {has: key => key in data, get: key => data[key]};
    }};
    if (name === 'ody') return {d: {overload: () => ({call: (_, value) => JSON.stringify(value)})}};
    if (name === 'odo') return {c: {overload: () => method}};
    throw new Error(name);
}};
vm.runInNewContext(fs.readFileSync(__dirname + '/capture.js', 'utf8'), {
    Java: java, console: {log: x => logs.push(x), error: x => logs.push(x)}
});
hook = method.implementation;
const args = ['https://example.invalid/dataQuery/health/getHealthDataByVersion', {},
    '{"version":0,"token":"DO_NOT_EXPORT"}', {getName: () => 'TestRsp'}];
assert.equal(hook(...args), result);
assert.equal(logs.length, 2);
assert(!logs.join('\n').includes('DO_NOT_EXPORT'));
assert.equal(JSON.parse(logs[1].split('HUAWEI_ARCHIVE ')[1]).request.version, '0');
hook('/dataSync/health/deleteAllHealthData', {}, '{}', args[3]);
assert.equal(logs.length, 2);
assert.equal(hook(args[0], {}, 'invalid-json', args[3]), result);
assert(logs[2].startsWith('HUAWEI_ARCHIVE_ERROR'));
error = new Error('original failure');
assert.throws(() => hook(...args), e => e === error);
console.log('PASS: response preserved, secret excluded, writes ignored, failures visible');
