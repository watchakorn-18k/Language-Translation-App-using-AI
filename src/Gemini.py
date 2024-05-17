import google.generativeai as genai
import os
import json


class GeminiAPI:
    def create_file_json(self):
            with open("api_gemini.json", "w") as outfile:
                json.dump(
                    {
                         "api_key": "get API key for the Gemini go to https://aistudio.google.com/app/apikey"
                    },
                    outfile,
                )

    def check_has_json(self):
        if not os.path.exists("api_gemini.json"):
            return self.create_file_json()
        
    def prompt_gemini_run(self, prompt : str) -> str:
        self.check_has_json()
        with open("api_gemini.json", "r") as f:
            gemini_data = json.load(f)
        if (
            gemini_data["api_key"]
            != "get API key for the Gemini go to https://aistudio.google.com/app/apikey"):
            api_key = gemini_data["api_key"]

            genai.configure(api_key=api_key)
            model = genai.GenerativeModel('gemini-1.0-pro-latest')
            response = model.generate_content(f"{prompt} translate to english only answer in json format is"
                + '{"translation_result": "<result>"}')
            return json.loads(response.text)["translation_result"]
        else:
            return "Please add the gemini key to the `api_gemini.json` file first."