op dropbox-search(query: String) -> Any
  description "Search file and folder names across the account, returning the first page of matches. Dropbox matches on name, not file content. Dropbox routes this read through POST, so it is declared as a write. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error_summary` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.dropboxapi.com"
  url = fmt("{base}/2/files/search_v2")
  content_type = "application/json"
  payload = { query }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
