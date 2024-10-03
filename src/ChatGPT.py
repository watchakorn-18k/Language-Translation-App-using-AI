import openai


class ChatGPTAPI:
    def __init__(self, api_key):
        openai.api_key = api_key

    def generate_response(self, prompt, model_engine="gpt-4o-mini"):
        completion = openai.chat.completions.create(
            model=model_engine,
            temperature=0.1,
            max_tokens=2000,
            messages=[
                {
                    "role": "system",
                    "content": """You are a translation expert. Translate into the target language while preserving the original sentence structure and meaning exactly.Translate in a neutral way, . try to preserve unique name such as company's name or brand's name.""",
                },
                {
                    "role": "user",
                    "content": "original_text:"
                    + prompt
                    + """\n Your translation in english:""",
                },
            ],
        )
        response = completion.choices[0].message.content
        return response
