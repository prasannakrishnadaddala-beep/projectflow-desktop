# ProjectFlow — Railway + GitLab CI/CD + Electron Desktop

## How it works

```
git push origin main  →  GitLab CI/CD
  1. test    syntax check app.py
  2. deploy  Railway CLI → live in ~60s
  3. build   Electron .AppImage / .exe with Railway URL baked in
```

## Step 1 — Create Railway project

1. Sign up at https://railway.app (free tier works)
2. New Project → Deploy from repo → select this repo
3. Railway detects Python via Procfile and deploys automatically
4. Service → Settings → Public Networking → Generate Domain
5. Copy the URL: https://xyz.up.railway.app

## Step 2 — Set GitLab CI/CD variables

Settings → CI/CD → Variables:

| Variable          | Value                                    | Masked |
|-------------------|------------------------------------------|--------|
| RAILWAY_TOKEN     | Railway Account Settings → Tokens        | yes    |
| RAILWAY_SERVICE   | Service ID from Railway dashboard URL    | yes    |
| RAILWAY_PROJECT   | Project ID from Railway dashboard URL    | yes    |
| APP_URL           | https://xyz.up.railway.app               |        |
| BUILD_ELECTRON    | true  (optional — builds installers too) |        |

## Step 3 — Push to deploy

```bash
git push origin main
```

## Step 4 — Run the Electron app

```bash
cd electron && npm install

# Dev mode pointing at Railway:
PROJECTFLOW_URL=https://xyz.up.railway.app npm start

# Or just run — it will prompt for your Railway URL on first launch
npm start
```

Change the URL anytime via File → Change Server URL in the app menu.

## URL resolution order (Electron)

1. PROJECTFLOW_URL environment variable
2. electron/config.json → backendUrl  (written by CI at build time)
3. Prompt user on first launch (saved to config.json)

## Project structure

```
app.py            Flask backend (deployed to Railway)
Procfile          how Railway starts the app
requirements.txt  Python deps
gunicorn.conf.py  production server config
railway.json      Railway service config
.gitlab-ci.yml    CI/CD pipeline
electron/
  main.js         Electron entry point
  preload.js      context bridge
  splash.html     loading screen
  config.json     backend URL config file
  package.json    electron-builder config
  assets/         app icons
```
