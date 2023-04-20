import asyncio
import json
from EdgeGPT import Chatbot, ConversationStyle
import re
import os


class BingChat:
    def __init__(self) -> None:
        self.text_original = "或许 时空法术"
        self.result_str = ""

    def create_file_json(self):
        with open("cookies.json", "w") as outfile:
            outfile.write(
                "You can see more https://github.com/watchakorn-18k/Language-Translation-App-using-CHATGPT#how-to-use-bing-chat"
            )

    async def main(self):
        if not os.path.exists("cookies.json"):
            return self.create_file_json()
        else:
            with open("./cookies.json", "r") as f:
                data_cookies = f.readline()
            if (
                data_cookies
                != "You can see more https://github.com/watchakorn-18k/Language-Translation-App-using-CHATGPT#how-to-use-bing-chat"
            ):
                with open("./cookies.json", "r") as f:
                    cookies = json.load(f)
                bot = Chatbot(cookies=cookies)
                data = await bot.ask(
                    prompt=f"'{self.text_original}' translate to english only answer in json format is "
                    + "{'translation_result': '<result>'}",
                    conversation_style=ConversationStyle.creative,
                    wss_link="wss://sydney.bing.com/sydney/ChatHub",
                )
                json_str = re.search(
                    r"\{.*\}", data["item"]["messages"][1]["text"]
                ).group()
                json_dict = json.loads(json_str)
                self.result_str = json_dict["translation_result"]
                await bot.close()
            else:
                self.result_str = "You need to add information to `cookies.json`"

    def get_data(self):
        return self.result_str

    if __name__ == "__main__":
        asyncio.run(main())
