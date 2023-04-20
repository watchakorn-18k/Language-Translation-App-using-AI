import json
from Bard import Chatbot
import os


class Bard:
    def __init__(self) -> None:
        self.text_input = ""
        if not os.path.exists("cookies_bard.json"):
            return self.create_file_json()

    def create_file_json(self):
        with open("cookies_bard.json", "w") as outfile:
            json.dump(
                {
                    "cookie_bard": "Session: Go to Application → Cookies → __Secure-1PSID. Copy the value of that cookie."
                },
                outfile,
            )

    def main(self):
        with open("cookies_bard.json", "r") as f:
            cookies = json.load(f)

        token = cookies["cookie_bard"]
        chatbot = Chatbot(token)

        data = chatbot.ask(
            f"{self.text_input} translate to english only answer in json format is"
            + '{"translation_result": "<result>"}'
        )

        data = data["content"]
        start_index = data.find("{")
        end_index = data.find("}") + 1

        self.json_string = (
            json.loads(data[start_index:end_index])["translation_result"]
            if data[start_index:end_index] != ""
            else "Can't translate"
        )

    def get_result(self):
        return self.json_string
