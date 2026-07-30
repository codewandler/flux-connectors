op notion-search(query: String) -> Any
  description "Search the pages and databases shared with this integration by title, returning the first page of matches (up to 100). Notion matches on title only — it does not search page text. Notion routes this read through POST, so it is declared as a write. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.notion.com"
  url = fmt("{base}/v1/search")
  content_type = "application/json"
  Notion_Version = "2022-06-28"
  payload = { query }
  response = http.request(body: payload, headers: { "Notion-Version": Notion_Version, "content-type": content_type }, method: "POST", url)
  return response
