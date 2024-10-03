import google.generativeai as genai
import os
import json


class GeminiAPI:
    def create_file_json(self):
        with open("api_gemini.json", "w") as outfile:
            json.dump(
                {
                    "api_key": "get API key for the Gemini go to https://aistudio.google.com/app/apikey",
                    "model": "gemini-1.5-flash",
                },
                outfile,
            )

    def check_has_json(self):
        if not os.path.exists("api_gemini.json"):
            return self.create_file_json()

    def prompt_gemini_run(self, prompt: str) -> str:
        self.check_has_json()
        with open("api_gemini.json", "r") as f:
            gemini_data = json.load(f)
        if (
            gemini_data["api_key"]
            != "get API key for the Gemini go to https://aistudio.google.com/app/apikey"
        ):
            api_key = gemini_data["api_key"]
            if gemini_data["model"] == "":
                gemini_data["model"] = "gemini-1.5-flash"
            genai.configure(api_key=api_key)
            model = genai.GenerativeModel(gemini_data["model"])
            response = model.generate_content(
                f"You are a translation expert. Translate into the target language while preserving the original sentence structure and meaning exactly.Translate in a neutral way, . try to preserve unique name such as company's name or brand's name , this is the prompt:'{prompt}' translate to english only answer in json format is"
                + '{"translation_result": "<result>"}',
            )
            text = response.text
            print("Debug: ", text)
            try:
                import re

                json_match = re.search(r"(\{.*\})", text)
                if json_match:
                    json_text = json_match.group(1)  # Extract the JSON content
                    print("Debug: ", json_text)  # Optional for debugging
                    return json.loads(json_text)["translation_result"]
                return text
            except Exception:
                return text
        else:
            return "Please add the gemini key to the `api_gemini.json` file first."
