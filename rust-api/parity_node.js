// 对拍脚本：输出 Node 侧固定输入的基准值，用于校验 Rust 移植的正确性
// 运行: node rust/parity_node.js  (在 api 根目录)
const path = require('path');
const root = path.join(__dirname, '..');
const { cryptoMd5, cryptoSha1, cryptoAesEncrypt, cryptoAesDecrypt, playlistAesEncrypt, playlistAesDecrypt, cryptoRSAEncrypt } = require(path.join(root, 'util/crypto'));
const { signatureWebParams, signatureAndroidParams, signParams, signParamsKey, signKey, signCloudKey } = require(path.join(root, 'util/helper'));
const { decodeLyrics, calculateMid, cookieToJson } = require(path.join(root, 'util/util'));

const out = {};

// md5 / sha1
out.md5_hello = cryptoMd5('hello');
out.md5_empty = cryptoMd5('');
out.md5_obj = cryptoMd5(JSON.stringify({ a: 1, b: 'x' }));
out.sha1_hello = cryptoSha1('hello');

// aes 默认路径（无显式 key/iv）
const aes1 = cryptoAesEncrypt('{"a":1}');
out.aes_default = { str: aes1.str, key: aes1.key };
out.aes_default_roundtrip = cryptoAesDecrypt(aes1.str, aes1.key);

// aes 显式 key/iv 路径（源码 bug：opt.key 存在时直接返回裸 hex 字符串）
const KEY32 = '90b8382a1bb4ccdcf063102053fd75b8';
const IV16 = 'f063102053fd75b8';
const aes2 = cryptoAesEncrypt('hello world', { key: KEY32, iv: IV16 });
out.aes_explicit = aes2; // 应为字符串（裸 hex）
out.aes_explicit_typeof = typeof aes2;

// aes decrypt 带显式 iv
out.aes_explicit_dec = cryptoAesDecrypt(aes2, KEY32, IV16);

// 显式 key 但无 iv 的分支（key 走 md5 派生，返回裸 hex —— bug 分支）
const aes3 = cryptoAesEncrypt('branch3', { key: 'fixedkey' });
out.aes_key_only = aes3;
out.aes_key_only_typeof = typeof aes3;
out.aes_key_only_dec = cryptoAesDecrypt(aes3, 'fixedkey');

// playlist aes
const pl = playlistAesEncrypt('hello playlist');
out.playlist = pl;
out.playlist_roundtrip = playlistAesDecrypt({ key: pl.key, str: pl.str });

// RSA raw（确定性：无随机 padding）
out.rsa_raw = cryptoRSAEncrypt('testdata123');

// signatures（固定 clienttime 等参数保证确定性）
const paramsA = { appid: 1005, clientver: 20489, clienttime: 1700000000, mid: 'abc', keyword: '周杰伦', obj: { x: 1 }, arr: [1, 2], nul: null, bool: true };
out.sig_android = signatureAndroidParams(paramsA);
out.sig_web = signatureWebParams(paramsA);
out.sign_params = signParams(paramsA, '{"d":1}');
out.sign_params_key = signParamsKey('{"a":1}', 1005, 20489);
out.sign_key = signKey('somehash', 'somemid', 12345, 1005);
out.sign_cloud_key = signCloudKey('HASH123', 999);
// decodeLyrics 用一个真实 krc base64 较难，此处仅验证 calculateMid/cookie；lyric 走集成测试

// calculateMid
out.mid = calculateMid();

// cookieToJson
out.cookie = cookieToJson('token=abc; userid=123; vip_type=6');

console.log(JSON.stringify(out, null, 2));
