use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CustomerId(String);

impl CustomerId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CustomerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerchantId(String);

impl MerchantId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerchantConnectorId(String);

impl MerchantConnectorId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentMethodId(String);

impl PaymentMethodId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardNumber(String);

impl CardNumber {
    pub fn new(number: String) -> Result<Self, String> {
        if number.is_empty() {
            return Err("Card number cannot be empty".to_string());
        }
        Ok(Self(number))
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpiryMonth(String);

impl ExpiryMonth {
    pub fn new(month: String) -> Result<Self, String> {
        let m = month.parse::<u8>().map_err(|_| "Invalid month")?;
        if !(1..=12).contains(&m) {
            return Err("Month must be between 1 and 12".to_string());
        }
        Ok(Self(month))
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpiryYear(String);

impl ExpiryYear {
    pub fn new(year: String) -> Result<Self, String> {
        if year.len() != 4 {
            return Err("Year must be 4 digits".to_string());
        }
        year.parse::<u16>().map_err(|_| "Invalid year")?;
        Ok(Self(year))
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BatchNumber(pub usize);

impl BatchNumber {
    pub fn new(num: usize) -> Self {
        Self(num)
    }

    pub fn value(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LineNumber(pub usize);

impl LineNumber {
    pub fn new(num: usize) -> Self {
        Self(num)
    }

    pub fn value(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(key: String) -> Self {
        Self(key)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint(String);

impl ApiEndpoint {
    pub fn new(endpoint: String) -> Result<Self, String> {
        url::Url::parse(&endpoint).map_err(|e| format!("Invalid URL: {}", e))?;
        Ok(Self(endpoint))
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldName(String);

impl FieldName {
    pub fn new(name: String) -> Self {
        Self(name)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl From<&str> for FieldName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldValue(String);

impl FieldValue {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl From<String> for FieldValue {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for FieldValue {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email(String);

impl Email {
    pub fn new(email: String) -> Result<Self, String> {
        if !email.contains('@') {
            return Err("Invalid email format".to_string());
        }
        Ok(Self(email))
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDirectory(std::path::PathBuf);

impl OutputDirectory {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self(path)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponseDirectory(std::path::PathBuf);

impl BatchResponseDirectory {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self(path)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BatchSize(usize);

impl BatchSize {
    pub fn new(size: usize) -> Result<Self, String> {
        if size == 0 {
            return Err("Batch size must be greater than 0".to_string());
        }
        Ok(Self(size))
    }

    pub fn value(&self) -> usize {
        self.0
    }
}
