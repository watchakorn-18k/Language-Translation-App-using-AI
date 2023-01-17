import flet as ft


def main(page: ft.Page):
    page.title = "แอพแปลภาษาด้วย CHATGPT | wk-18k"
    window_width, window_height = (400, 725)
    page.window_width = window_width
    page.window_height = window_height
    page.window_always_on_top = True
    # page.window_skip_task_bar = True
    input_text = ft.TextField()
    pr = ft.ProgressBar(width=300)
    pr.value = 0

    def translating(e):
        data = input_text.value
        if data != "":
            pr.value = None
            page.update()
            import openai
            import re

            # ใส่ token คีย์ API
            openai.api_key = "API key for the CHATGPT"

            model_engine = "text-davinci-003"

            # สร้างตัวตอบกลับ
            completion = openai.Completion.create(
                engine=model_engine,
                prompt=f"{data} translate to write in words english",
                max_tokens=1024,
                n=1,
                stop=None,
                temperature=0.5,
            )

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
            result.value = "Fill in the text box"
            result2.value = "กรอกข้อความลงในช่องป้อนข้อมูล"
            page.update()

    cal_btn = ft.ElevatedButton(text="แปลภาษา", width=300, on_click=translating)
    result = ft.Text("", width=250)
    result2 = ft.Text("", width=250)

    def copying(e):
        page.set_clipboard(result.value)
        page.snack_bar = ft.SnackBar(
            content=ft.Text("คัดลอกแล้ว"),
        )
        page.snack_bar.open = True
        page.update()

    copy_btn = ft.IconButton(icon=ft.icons.COPY, on_click=copying)
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

    copy_btn2 = ft.IconButton(icon=ft.icons.COPY, on_click=copying2)
    content2 = ft.Row(
        [
            result2,
            copy_btn2,
        ],
        alignment=ft.MainAxisAlignment.END,
    )

    page.add(
        ft.Row(
            [
                ft.Column(
                    [
                        ft.Text(
                            value="แอพแปลภาษาด้วย CHATGPT dev wk-18k",
                            text_align="center",
                        ),
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
        )
    )


ft.app(target=main)
