<p align="center"><img src="src\icon.ico" width=50></p>

# 🌍 Language Translation App using CHATGPT , BING CHAT and BARD 🗣️

<p align="center"><img src="https://cdn.discordapp.com/attachments/585068497495654413/1067843291745370263/gamedfdsf.gif">
</p>

This app utilizes the power of the CHATGPT language model to provide real-time translation of text input in Thai and English.

- Support for Thai and English languages
- Easy-to-use interface

<p align="center"><img src="https://cdn.discordapp.com/attachments/581018943041306641/1098513058206326815/gamedfdsf.gif" width=300></p>

## How to use

1. Run Application 1 time
2. Then go to file `api_key_chatgpt.json` and insert the API KEY, get it from [https://beta.openai.com/](https://beta.openai.com/), then save and close the file.
   ![img](https://cdn.discordapp.com/attachments/585068497495654413/1067831416995455067/gamedfdsf.gif)
3. Run Application
4. Enter text in any language in the text field.
5. Press the <img src="https://media.discordapp.net/attachments/585068497495654413/1067750218969399426/image-removebg-preview.png" width="30"> button to get the translated text

**IMPORTANT**: Remember to enter your API key for the CHATGPT model before running the app in file `api_key_chatgpt.json`

## OCR (Optical Character Recognition)

can be used to extract text from images or scanned documents, including text on the screen. Currently, our OCR system supports two languages: Chinese and English.

<p align="center"><img src="https://cdn.discordapp.com/attachments/585068497495654413/1067751122707689502/Page_1.png" height=400>
<img src="https://cdn.discordapp.com/attachments/585068497495654413/1067796812540424283/gamedfdsf.gif" height=390></p>

**NOTE**: You can press the keyboard shortcut `Ctrl + Shift + Z` to perform OCR.

## Settings Menu

<p align="center">
<img src="https://cdn.discordapp.com/attachments/581018943041306641/1098350494969253928/image.png" height="400">
<img src="https://cdn.discordapp.com/attachments/581018943041306641/1098350257651335299/image.png" height="400">
</p>

## How to use Bing Chat

- Checking access (Required) [here](https://github.com/acheong08/EdgeGPT#checking-access-required)
- Install the cookie editor extension for [Chrome](https://chrome.google.com/webstore/detail/cookie-editor/hlkenndednhfkekhgcdicdfddnkalmdm) or [Firefox](https://addons.mozilla.org/en-US/firefox/addon/cookie-editor/)
- Go to `bing.com`
- Open the extension
- Click "Export" on the bottom right, then "Export as JSON" (This saves your cookies to clipboard)
- Paste your cookies into a file `cookies_bing_chat.json`

Example `cookies_bing_chat.json`
```diff
- You can see more https://github.com/watchakorn-18k/Language-Translation-App-using-CHATGPT#how-to-use-bing-chat

+ [
+    {
+        "domain": ".bing.com",
+        "expirationDate": ..............
```

You can see more [here](https://github.com/acheong08/EdgeGPT/blob/master/docs/README.md#getting-authentication-required).

<p align="center">
<img src="https://cdn.discordapp.com/attachments/581018943041306641/1098685947480113336/image.png" height="400">
<img src="https://cdn.discordapp.com/attachments/581018943041306641/1098694722752614420/image.png" height="400">

</p>

## How to use Bard

- Go to https://bard.google.com/
- F12 for console
- Copy the values
  - Session: Go to Application → Cookies → \_\_Secure-1PSID. Copy the value of that cookie.
  - Paste it into a file `cookies_bard.json`
  
Example `cookies_bard.json`
```diff
- {"cookie_bard": "Session: Go to Application > Cookies > __Secure-1PSID. Copy the value of that cookie."}

+ {"cookie_bard": "VA............................................"}
```

## Installation

1. Make sure you have Python 3 installed on your system
2. Install package `pip install fenv`
3. Clone the repository and create virtualenv with `fenv clone https://github.com/watchakorn-18k/Language-Translation-App-using-CHATGPT`
4. Run the app with the command `python app.py` of `flet app.py`

## Development

This app is open-source and can be further developed. Feel free to make pull requests or suggest new features.

## Build

```
pyinstaller app.spec
```

## Technical details

The app is built using the OpenAI's CHATGPT model, which is a powerful language generation model. It uses the Hugging Face's transformers library to fine-tune the model and make it work for our specific use case.

## Credit

Snipper source code from [textshot](https://github.com/ianzhao05/textshot/blob/master/textshot/textshot.py)
