import flet as ft

from src.Snippers import main_snipper
from src.Apikey import ApikeyGPT
import pyperclip
import keyboard


def main(page: ft.Page):
    ApikeyGPT().api_key_GPT()
    page.title = "แอพแปลภาษาด้วย CHATGPT | wk-18k"
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
        title=ft.Text("แอพแปลภาษาด้วย CHATGPT dev wk-18k", size=15),
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
        value=ch_eng,
        tooltip="เลือกภาษาที่จะ OCR",
    )

    def Snipper(e):
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

    snipper_btn = ft.IconButton(
        ft.icons.CROP, on_click=Snipper, tooltip="OCR (Ctrl+Shift+Z)"
    )
    keyboard.add_hotkey("ctrl+shift+z", Snipper)

    def translating(e):
        apikey = ApikeyGPT().get_api_key()
        data = input_text.value
        if data != "":
            pr.value = None
            page.update()
            import openai
            import re

            # ใส่ token คีย์ API
            openai.api_key = apikey

            model_engine = "text-davinci-003"

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
            except:
                result.value = "Invalid API key"
                result2.value = "API Key"
                pr.value = 0
                page.update()

            response = completion.choices[0].text
            output = re.sub(r"^\s+|\s+$", "", response)
            from googletrans import Translator

            translator = Translator()
            pr.value = 0
            page.update()
            result.value = output
            page.update()
            result2.value = translator.translate(src="en", dest="th", text=output).text
            page.update()
        else:
            result.value = (
                "Invalid API Key"
                if apikey == "API key for the CHATGPT"
                else "Fill in the text box"
            )
            result2.value = (
                "API Key ไม่ถูกต้อง"
                if apikey == "API key for the CHATGPT"
                else "กรอกข้อความลงในช่องป้อนข้อมูล"
            )
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
        [
            result,
            copy_btn,
        ],
        alignment=ft.MainAxisAlignment.END,
    )

    def copying2(e):
        page.set_clipboard(result2.value)
        page.snack_bar = ft.SnackBar(
            content=ft.Text("คัดลอกแล้ว"),
        )
        page.snack_bar.open = True
        page.update()

    copy_btn2 = ft.IconButton(
        icon=ft.icons.COPY, on_click=copying2, tooltip="คลิกเพื่อคัดลอก"
    )
    content2 = ft.Row(
        [
            result2,
            copy_btn2,
        ],
        alignment=ft.MainAxisAlignment.END,
    )

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
                                ],
                                horizontal_alignment="center",
                            )
                        ],
                        alignment=ft.MainAxisAlignment.CENTER,
                    ),
                    ft.Row([snipper_btn, dropdown]),
                ],
                alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
            ),
            height=600,
        )
    )


ft.app(target=main)
