import fs from 'node:fs';
const p='src-tauri/gen/android/app/src/main/AndroidManifest.xml';
if(!fs.existsSync(p)) process.exit(0);
let s=fs.readFileSync(p,'utf8');
if(!s.includes('android.permission.INTERNET')) s=s.replace(/<manifest([^>]*)>/, '<manifest$1>\n    <uses-permission android:name="android.permission.INTERNET" />');
fs.writeFileSync(p,s);
