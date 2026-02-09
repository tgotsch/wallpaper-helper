use std::path::*;
use crate::WallpaperData;

pub struct SourceData
{
    local_path: String,
    slideshow_capable: bool,
    slideshow_enabled: bool,
}

pub trait WallpaperSource
{
    fn get_image_path(&self) -> String;
    fn prepare_image(&mut self);
    fn slideshow_capable(&self) -> bool;
    fn slideshow_enabled(&self) -> bool;
}

pub struct LocalFileSource
{
    data: SourceData,
}

impl WallpaperSource for LocalFileSource
{
    fn get_image_path(&self) -> String
    {
        return self.data.local_path;
    }

    fn prepare_image(&mut self)
    {
        return;
    }

    fn slideshow_capable(&self) -> bool
    {
        return self.data.slideshow_capable;
    }

    fn slideshow_enabled(&self) -> bool
    {
        return self.data.slideshow_enabled;
    }
}

pub struct UrlSource
{
    data: SourceData,
    remote_path: String,
}

impl WallpaperSource for UrlSource
{
    fn get_image_path(&self) -> String
    {
        return self.data.local_path;
    }

    fn prepare_image(&mut self)
    {
        self.data.local_path = download_image_to_temp(self.remote_path);
        return;
    }

    fn slideshow_capable(&self) -> bool
    {
        return self.data.slideshow_capable;
    }

    fn slideshow_enabled(&self) -> bool
    {
        return self.data.slideshow_enabled;
    }
}

pub struct FolderSource
{
    data: SourceData,
    curr_index: usize,
}

impl WallpaperSource for FolderSource
{
    fn get_image_path(&self) -> String
    {
        
    }
}

pub struct BooruSource
{

}