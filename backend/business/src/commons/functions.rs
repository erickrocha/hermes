
use uuid::Uuid;

pub fn bytes_para_string(bytes: Vec<u8>) -> String {
	Uuid::from_slice(&bytes)
		.map(|u| u.to_string())
		.unwrap_or_default() 
}

pub fn string_to_bytes(uuid_str: &str) -> Vec<u8> {
	Uuid::parse_str(uuid_str)
		.map(|u| u.as_bytes().to_vec())
		.unwrap_or_else(|_| vec![0; 16])
}