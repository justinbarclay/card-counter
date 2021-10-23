resource "aws_s3_bucket" "card-counter" {
  bucket = "card-counter.slack"
  acl    = "public-read"
  policy = file("policy.json")

  website {
    index_document = "index.html"
    error_document = "index.html"
  }
}
