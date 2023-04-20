import flet as ft

from src.Snippers import main_snipper
from src.Apikey import ApikeyGPT
import pyperclip
import keyboard
from googletrans import Translator, LANGCODES, LANGUAGES
import asyncio
from src.Bingchat import BingChat
from src.Bard import Bard
import datetime


def main(page: ft.Page):
    global translator_google_lang
    global translator_lang
    translator_lang = "Bard"
    translator_google_lang = "thai"
    ApikeyGPT().api_key_GPT()
    Bard().check_has_json()
    BingChat().check_has_json()
    page.title = "แอพแปลภาษาด้วย AI | wk-18k"
    window_width, window_height = (400, 725)
    page.window_width = window_width
    page.window_height = window_height
    page.window_always_on_top = True
    # page.window_skip_task_bar = True
    input_text = ft.TextField()
    page.theme = page.dark_theme = ft.Theme(font_family="Kanit")
    pr = ft.ProgressBar(width=300)
    page.theme = ft.Theme(font_family="Kanit")
    pr.value = 0
    page.appbar = ft.AppBar(
        leading_width=40,
        title=ft.Column(
            [
                ft.Text("แอพแปลภาษาด้วย AI", size=20),
            ]
        ),
        center_title=True,
    )
    ch_eng = "Chinese and English"
    th_eng = "Thai and English"
    dropdown = ft.Dropdown(
        hint_text=ch_eng,
        options=[
            ft.dropdown.Option(ch_eng),
            ft.dropdown.Option(th_eng),
        ],
        width=200,
        value=ch_eng,
        tooltip="เลือกภาษาที่จะ OCR",
    )

    def Snipper(e=None):
        pr.value = None
        result.value = "OCR Scanning text..."
        result2.value = "OCR กำลังสแกนข้อความ..."
        page.update()
        if dropdown.value == "Chinese and English":
            main_snipper(["ch_sim", "en"])
        elif dropdown.value == "Thai and English":
            main_snipper(["th", "en"])

        input_text.value = pyperclip.paste()
        pr.value = 0
        result.value = "Scan completed, press Translate button to translate."
        result2.value = "สแกนเสร็จแล้ว กดปุ่มแปลภาษา เพื่อแปลภาษา"
        page.update()

    keyboard.add_hotkey("ctrl+shift+z", Snipper)

    def settings_menu(e):
        def close_dlg(e):
            dlg_modal.open = False
            page.update()

        def set_transtalor(e):
            global translator_lang
            translator_lang = dropdown_translator.value
            close_dlg(e)

        translator_list = ["Google Translate", "ChatGPT", "Bing Chat", "Bard"]
        dropdown_translator = ft.Dropdown(
            hint_text=translator_lang,
            width=160,
            value=translator_lang,
            tooltip="ตัวแปลภาษา",
        )
        for i in translator_list:
            dropdown_translator.options.append(ft.dropdown.Option(i))
        dlg_modal = ft.AlertDialog(
            modal=True,
            title=ft.Text("Settings"),
            content=ft.Column(
                [
                    ft.Row(
                        [ft.Text("Translator"), dropdown_translator],
                        alignment=ft.MainAxisAlignment.CENTER,
                    )
                ],
                height=100,
                alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
            ),
            actions=[
                ft.TextButton("Accept", on_click=set_transtalor),
                ft.TextButton("Cancel", on_click=close_dlg),
            ],
            content_padding=20,
            actions_alignment=ft.MainAxisAlignment.END,
            on_dismiss=lambda e: print("Modal dialog dismissed!"),
        )

        def open_dlg_modal(e):
            page.dialog = dlg_modal
            dlg_modal.open = True
            page.update()

        open_dlg_modal(e)

    snipper_btn = ft.IconButton(
        ft.icons.CROP, on_click=Snipper, tooltip="OCR (Ctrl+Shift+Z)"
    )
    settings_btn = ft.IconButton(
        ft.icons.SETTINGS, on_click=settings_menu, tooltip="Settings"
    )

    def translating(e):
        apikey = ApikeyGPT().get_api_key()
        data = input_text.value
        translator = Translator()
        content.visible = False
        content2.visible = False
        change_langauge_btn.visible = False
        page.update()
        if data != "":
            pr.value = None
            page.update()
            import openai
            import re

            # ใส่ token คีย์ API
            openai.api_key = apikey

            model_engine = "text-davinci-003"

            if translator_lang == "ChatGPT":
                # สร้างตัวตอบกลับ
                try:
                    completion = openai.Completion.create(
                        engine=model_engine,
                        prompt=f"{data} translate to write in words english",
                        max_tokens=1024,
                        n=1,
                        stop=None,
                        temperature=0.5,
                    )
                    response = completion.choices[0].text
                except:
                    content.visible = True
                    content2.visible = True
                    change_langauge_btn.visible = True
                    result.value = "Invalid API key ChatGPT"
                    result2.value = "API Key ChatGPT in `api_key_chatgpt.json`"
                    pr.value = 0
                    page.update()

            elif translator_lang == "Google Translate":
                response = translator.translate(dest="en", text=data).text

            elif translator_lang == "Bing Chat":
                try:
                    bingchat = BingChat()
                    bingchat.text_original = data
                    asyncio.run(bingchat.main())
                    response = bingchat.get_data()
                except KeyError:
                    response = "can't translate"

            elif translator_lang == "Bard":
                try:
                    bard = Bard()
                    bard.text_input = data
                    bard.main()
                    response = bard.get_result()
                except KeyError:
                    response = "can't translate"

            output = re.sub(r"^\s+|\s+$", "", response)
            pr.value = 0

            page.update()
            result.value = output
            page.update()
            result2.value = translator.translate(
                src="en", dest=LANGCODES[translator_google_lang], text=output
            ).text
            content.visible = True
            content2.visible = True
            change_langauge_btn.visible = True
            page.update()

        else:
            result.value = "Fill in the text box"
            result2.value = translator.translate(
                src="en", dest=LANGCODES[translator_google_lang], text=result.value
            ).text
            content.visible = True
            content2.visible = True
            change_langauge_btn.visible = True
            page.update()

    cal_btn = ft.ElevatedButton(
        content=ft.Icon(ft.icons.TRANSLATE),
        width=300,
        on_click=translating,
        tooltip="คลิกเพื่อแปลภาษา",
    )
    result = ft.Text("", width=250)
    result2 = ft.Text("", width=250)

    def copying(e):
        page.set_clipboard(result.value)
        page.snack_bar = ft.SnackBar(
            content=ft.Text("คัดลอกแล้ว"),
        )
        page.snack_bar.open = True
        page.update()

    copy_btn = ft.IconButton(
        icon=ft.icons.COPY, on_click=copying, tooltip="คลิกเพื่อคัดลอก"
    )
    content = ft.Row(
        [result, copy_btn],
        alignment=ft.MainAxisAlignment.END,
        visible=False,
    )

    def copying2(e):
        page.set_clipboard(result2.value)
        page.snack_bar = ft.SnackBar(
            content=ft.Text("คัดลอกแล้ว"),
        )
        page.snack_bar.open = True
        page.update()

    def change_lang(e):
        translator = Translator()

        def close_dlg(e):
            dlg_modal.open = False
            page.update()

        def set_transtalor(e):
            global translator_google_lang
            translator_google_lang = dropdown_translator.value
            result2.value = translator.translate(
                src="en", dest=LANGCODES[translator_google_lang], text=result.value
            ).text
            close_dlg(e)

        dropdown_translator = ft.Dropdown(
            hint_text=translator_google_lang,
            width=190,
            value=translator_google_lang,
            tooltip="เปลี่ยนภาษา",
        )
        if dropdown_translator.options == []:
            for i in LANGUAGES.values():
                dropdown_translator.options.append(ft.dropdown.Option(i))

        dlg_modal = ft.AlertDialog(
            modal=True,
            title=ft.Text("Change Language"),
            content=ft.Container(
                content=ft.Row(
                    [ft.Text("Language"), dropdown_translator],
                    alignment=ft.MainAxisAlignment.CENTER,
                ),
                padding=ft.padding.only(top=50, bottom=50),
            ),
            actions=[
                ft.TextButton("Accept", on_click=set_transtalor),
                ft.TextButton("Cancel", on_click=close_dlg),
            ],
            content_padding=5,
            actions_alignment=ft.MainAxisAlignment.END,
            on_dismiss=lambda e: print("Modal dialog dismissed!"),
        )

        def open_dlg_modal(e):
            page.dialog = dlg_modal
            dlg_modal.open = True
            page.update()

        open_dlg_modal(e)
        # print(LANGCODES)

    copy_btn2 = ft.IconButton(
        icon=ft.icons.COPY, on_click=copying2, tooltip="คลิกเพื่อคัดลอก"
    )
    content2 = ft.Row(
        [
            result2,
            copy_btn2,
        ],
        visible=False,
        alignment=ft.MainAxisAlignment.END,
    )

    change_langauge_btn = ft.IconButton(
        ft.icons.EDIT,
        on_click=change_lang,
        tooltip="เปลี่ยนภาษา",
        visible=False,
    )

    current_year = datetime.date.today().year
    last_year = current_year
    page.add(
        ft.Container(
            ft.Column(
                [
                    ft.Row(
                        [
                            ft.Column(
                                [
                                    input_text,
                                    cal_btn,
                                    pr,
                                    content,
                                    content2,
                                    change_langauge_btn,
                                ],
                                horizontal_alignment="center",
                            )
                        ],
                        alignment=ft.MainAxisAlignment.CENTER,
                    ),
                    ft.Column(
                        [
                            ft.Row(
                                [
                                    snipper_btn,
                                    dropdown,
                                    settings_btn,
                                ],
                                alignment=ft.MainAxisAlignment.SPACE_AROUND,
                            ),
                            ft.Row(
                                [
                                    ft.Markdown(
                                        f"""
พัฒนาโดย [wk18k © {last_year}](https://github.com/watchakorn-18k/Language-Translation-App-using-CHATGPT)\n
                        """
                                    ),
                                ],
                                alignment=ft.MainAxisAlignment.CENTER,
                            ),
                        ]
                    ),
                ],
                alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
            ),
            height=600,
        )
    )


ft.app(target=main)
