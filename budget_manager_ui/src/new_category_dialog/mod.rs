mod imp;

use crate::calender_button::CalendarButton;
use crate::transaction::transaction_object::TransactionObject;
use adw::glib::{clone, closure_local};
use adw::subclass::prelude::ObjectSubclassIsExt;
use budget_manager::budgeting::category::Category;
use chrono::{NaiveDate, NaiveDateTime};
use glib::Object;
use gtk::glib::DateTime;
use gtk::prelude::*;
use gtk::{glib, Button, Editable, Entry, ResponseType, SpinButton, StringList};

use crate::window::Window;

glib::wrapper! {
    pub struct NewCategoryDialog(ObjectSubclass<imp::NewCategoryDialog>)
    @extends gtk::Dialog, gtk::Window, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl NewCategoryDialog {
    pub fn new(parent: &Window, categories: Vec<Category>, category: Option<Category>) -> Self {
        let d: Self = Object::builder()
            .property("use-header-bar", 1.to_value())
            .property("destroy-with-parent", true.to_value())
            .build();
        d.set_transient_for(Some(parent));
        d.set_default_response(ResponseType::Accept);
        d.imp()
            .categories
            .replace(categories.iter().map(|v| v.name()).collect());
        d.imp().category.replace(category);
        d.imp().set_fields();
        d
    }
}
