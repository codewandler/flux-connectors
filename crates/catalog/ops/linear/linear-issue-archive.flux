op linear-issue-archive(id: String) -> Any
  description "Archive an issue. It leaves every list, board and search in Linear and stops appearing to the team. This is Linear's form of deletion; it can be undone from Linear's own UI but not by this connector, which declares no unarchive operation. Linear answers every failure with HTTP 200 and an `errors` array beside a null `data`, so check `errors` and the payload's `success` flag before treating the issue as archived"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """mutation IssueArchive($id: String!) {
  issueArchive(id: $id) {
    success
  }
}
"""
  payload = { query, variables: { id } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
