pub struct StringPool {

    data: Vec<String>

}

impl StringPool {

    pub fn new() -> Self {
        StringPool {
            data: vec![]
        }
    }

    pub fn insert(&self, str: String) -> &str {
        let mut this = unsafe {
            &mut *(self as *const Self as *mut Self)
        };
        this.data.push(str);
        &this.data.last().unwrap()[..]
    }

}