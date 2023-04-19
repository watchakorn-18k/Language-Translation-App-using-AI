<p align="center"><img src="src\icon.ico" width=50></p>

# 🌍 Language Translation App using CHATGPT 🗣️

<p align="center">Language Translation</p>
<p align="center"><img src="https://media.discordapp.net/attachments/585069498986397707/1064699087431143516/gamedfdsf.gif?width=374&height=671" width=300></p>

<p align="center">OCR (Optical Character Recognition)</p>
<p align="center"><img src="https://cdn.discordapp.com/attachments/585068497495654413/1067796812540424283/gamedfdsf.gif?width=374&height=671" width=300>
</p>

<p align="center">Example</p>
<p align="center"><img src="https://cdn.discordapp.com/attachments/585068497495654413/1067843291745370263/gamedfdsf.gif">
</p>

This app utilizes the power of the CHATGPT language model to provide real-time translation of text input in Thai and English.

- Support for Thai and English languages
- Easy-to-use interface

## How to use

1. Run Application 1 time
2. Then go to file `apikey.json` and insert the API KEY, get it from [https://beta.openai.com/](https://beta.openai.com/), then save and close the file.
   ![img](https://cdn.discordapp.com/attachments/585068497495654413/1067831416995455067/gamedfdsf.gif)
3. Run Application
4. Enter text in any language in the text field.
5. Press the <img src="https://media.discordapp.net/attachments/585068497495654413/1067750218969399426/image-removebg-preview.png" width="30"> button to get the translated text

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

## Installation

1. Make sure you have Python 3 installed on your system
2. Clone the repository or download the zip file
3. Run the command `pip install -r requirements.txt` to install the necessary dependencies
4. Run the app with the command `python app.py`

## Development

This app is open-source and can be further developed. Feel free to make pull requests or suggest new features.

## Build

```
pyinstaller app.spec
```

## Technical details

The app is built using the OpenAI's CHATGPT model, which is a powerful language generation model. It uses the Hugging Face's transformers library to fine-tune the model and make it work for our specific use case.

## Credis

Snipper source code from [textshot](https://github.com/ianzhao05/textshot/blob/master/textshot/textshot.py)

**IMPORTANT**: Remember to enter your API key for the CHATGPT model before running the app in file `apikey.json`
