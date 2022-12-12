install.packages("ggplot2")
install.packages("tidyverse")

library(ggplot2)
library(tidyverse)

master = as_tibble(read.csv("/home/marcus/CLionProjects/seng480b-group-project-rs/master.csv", stringsAsFactors = TRUE))
master$date = as.Date(as.POSIXct(master$seconds_since_epoch, origin="1970-01-01"))

var_additons = filter(filter(master,  diff_type=="Addition"), declaration_type=="Var")

ggplot(var_additons, aes(x=date, fill=project_name)) +
  scale_x_date(date_breaks = "12 month", date_labels =  "%Y") +
  facet_wrap(~project_name, ncol=1) +
  geom_histogram(bins=24) +
  scale_y_log10("Log Transformed var Declaration Additions") + 
  xlab("Year")

bar_data = master %>%
  filter(diff_type=="Addition") %>%
  group_by(project_name) %>%
  summarise(
    var = sum(declaration_type=="Var"),
    total = sum(diff_type=="Addition"), 
    explict = sum(declaration_type!="Var"),
    first_var_declaration=min(date) 
  ) %>%
  filter(var!=0)
   
bar_data
ggplot(bar_data, aes(fill=project_name, x = reorder(project_name, var/total), y=var/total)) +
  geom_bar(position="dodge", stat="identity") + 
  theme(axis.text.x = element_text(angle = 45, vjust = 0.5)) +
  ylab("Preportion of var Declarations") +
  xlab("Project Name")


