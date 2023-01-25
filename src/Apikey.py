import os
import json


class ApikeyGPT:
    def __init__(self) -> None:
        self.data = {"api_key": "API key for the CHATGPT"}

    def create_file_json(self):
        with open("apikey.json", "w") as outfile:
            json.dump(self.data, outfile)

    def api_key_GPT(self):
        """
        if file if non then create file json and what if have file then create file json
        """

        if not os.path.exists("apikey.json"):
            return self.create_file_json()
        else:
            return self.get_api_key()

    def get_api_key(self):
        with open("apiKey.json") as json_file:
            data = json.load(json_file)
        return data["api_key"]
