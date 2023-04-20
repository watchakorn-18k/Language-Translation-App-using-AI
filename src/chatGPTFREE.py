import http.client
import json

conn = http.client.HTTPSConnection("api.pawan.krd")
payload = json.dumps(
    {
        "model": "gpt-3.5-turbo",
        "messages": [
            {"role": "system", "content": "Act as a translator."},
            {
                "role": "user",
                "content": f"'圣灵大陆癸寅年阳月霜降' translate to english only answer in json format is"
                + '{"translation_result": "<result>"}',
            },
        ],
    }
)
headers = {
    "Content-Type": "application/json",
    "Authorization": "Bearer pk-tTDkeeoAqbJoqjFUOOKubzcVQgfWEJUQGLfobQJrVXAaKInJ",
}
conn.request("POST", "/v1/chat/completions", payload, headers)
res = conn.getresponse()
data = res.read()
result = json.loads(
    json.loads(data.decode("utf-8"))["choices"][0]["message"]["content"]
)
print(result["translation_result"])
