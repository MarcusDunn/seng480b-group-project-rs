library(ggplot2)
library(tidyverse)


master = as_tibble(read.csv("/home/marcus/CLionProjects/seng480b-group-project-rs/master.csv", stringsAsFactors = TRUE))

master$date = as.Date(as.POSIXct(master$seconds_since_epoch, origin="1970-01-01"))
master$date

master$date

ggplot(filter(filter(master,  diff_type=="Addition"), declaration_type=="Var"), aes(x=date, fill=project_name)) 
  + scale_x_date(date_breaks = "12 month", date_labels =  "%Y") 
  + geom_histogram(aes(y=cumsum(..count..)))

ggplot(filter(filter(master,  diff_type=="Addition"), declaration_type=="Var"), aes(x=date, fill=project_name)) +
  scale_x_date(date_breaks = "12 month", date_labels =  "%Y") +
  facet_wrap(~project_name, ncol=1) +
  geom_histogram(bins=24) +
  scale_y_log10("Log Transformed var Declaration Additions") + 
  xlab("Year")

ggplot(master, aes(x=indentation, fill=declaration_type)) +
  geom_histogram(binwidth = 1) +
  facet_wrap(~declaration_type, ncol=1, scale="free_y")

count(master)

mean(master$indentation)

t.test(filter(master, declaration_type=="Type")$indentation, filter(master, declaration_type=="Var")$indentation, var.equal = FALSE)

master_anova = lm(indentation ~ declaration_type, data=master)?
