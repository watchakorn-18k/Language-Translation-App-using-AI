import asyncio
import json
from EdgeGPT import Chatbot, ConversationStyle
import re
import os


class BingChat:
    def __init__(self, data) -> None:
        self.text_original = data
        self.result_str = ""

    def create_file_json(self):
        with open("cookies_bing_chat.json", "w") as outfile:
            outfile.write(
                "You can see more https://github.com/watchakorn-18k/Language-Translation-App-using-CHATGPT#how-to-use-bing-chat"
            )

    async def main(self):
        if not os.path.exists("cookies_bing_chat.json"):
            return self.create_file_json()
        else:
            with open("./cookies_bing_chat.json", "r") as f:
                data_cookies = f.readline()
            if (
                data_cookies
                != "You can see more https://github.com/watchakorn-18k/Language-Translation-App-using-CHATGPT#how-to-use-bing-chat"
            ):
                with open("./cookies_bing_chat.json", "r") as f:
                    cookies = json.load(f)
                bot = Chatbot(cookies=cookies)
                try:
                    data = await bot.ask(
                        prompt=f"'{self.text_original}' translate to english only answer in json format is "
                        + '{"translation_result": "<result>"}',
                        conversation_style=ConversationStyle.creative,
                        wss_link="wss://sydney.bing.com/sydney/ChatHub",
                    )
                except:
                    self.result_str = "`cookies_bing_chat.json` has expired or the data is invalid, please update it."
                await bot.close()
                result = str(data["item"]["messages"][1]["text"].encode("utf-8"))
                start_index = result.find("{")
                end_index = result.find("}") + 1
                self.result_str = json.loads(result[start_index:end_index])[
                    "translation_result"
                    if result[start_index:end_index] != ""
                    else "Can't translate"
                ]
            else:
                self.result_str = (
                    "You need to add information to `cookies_bing_chat.json`"
                )

    def get_data(self):
        return self.result_str

    if __name__ == "__main__":
        asyncio.run(main())
