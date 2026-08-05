op notion-database-query(database_id: String) -> Any
  description "Query a database and return its first page of entries (up to 100), unfiltered and in the database's default order. Each entry is a page, and its `properties` are keyed by that database's own column names. Direction remains conservatively authored as write pending individual review; POST is transport only and supplied no evidence. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.notion.com"
  url = fmt("{base}/v1/databases/{database_id}/query")
  Notion_Version = "2022-06-28"
  response = http.request(headers: { "Notion-Version": Notion_Version }, method: "POST", url)
  return response
