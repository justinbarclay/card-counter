set datafile separator ','
set xdata time
set timefmt '%d-%m-%y'
set format x "%d %b"
set autoscale x
set title "MMP"
plot for[col=2:3] "burndown.csv" u 1:col title columnheader(col) with lines